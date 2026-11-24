use crate::model::ParametricVar;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    Syntax(String),
    UnknownVar(String),
    Cycle(Vec<String>),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(m) => write!(f, "Syntax error: {m}"),
            Self::UnknownVar(v) => write!(f, "Unknown variable: {v}"),
            Self::Cycle(c) => write!(f, "Cyclic dependency: {}", c.join(" -> ")),
        }
    }
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Gt,
    Lt,
    Eq,
}

fn tokenize(input: &str) -> Result<Vec<Token>, DagError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let val = s
                .parse::<f64>()
                .map_err(|_| DagError::Syntax(format!("Invalid number `{s}`")))?;
            tokens.push(Token::Num(val));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' || ch == '@' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            tokens.push(Token::Ident(s));
            continue;
        }
        let tok = match ch {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '(' => Token::LParen,
            ')' => Token::RParen,
            ',' => Token::Comma,
            '>' => Token::Gt,
            '<' => Token::Lt,
            '=' => Token::Eq,
            _ => return Err(DagError::Syntax(format!("Unexpected character `{ch}`"))),
        };
        tokens.push(tok);
        i += 1;
    }
    Ok(tokens)
}

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    Eq,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, DagError> {
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> Result<Expr, DagError> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Some(Token::Gt) => BinOp::Gt,
                Some(Token::Lt) => BinOp::Lt,
                Some(Token::Eq) => BinOp::Eq,
                _ => break,
            };
            self.next();
            let rhs = self.parse_add()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, DagError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.next();
            let rhs = self.parse_mul()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, DagError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.next();
            let rhs = self.parse_unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, DagError> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.next();
            Ok(Expr::Neg(Box::new(self.parse_unary()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, DagError> {
        match self.next() {
            Some(Token::Num(v)) => {
                if let Some(Token::Ident(unit)) = self.peek() {
                    let mult = match unit.to_ascii_lowercase().as_str() {
                        "mm" | "deg" => Some(1.0),
                        "cm" => Some(10.0),
                        "in" => Some(25.4),
                        _ => None,
                    };
                    if let Some(m) = mult {
                        self.next();
                        return Ok(Expr::Num(v * m));
                    }
                }
                Ok(Expr::Num(v))
            }
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.next();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek(), Some(Token::Comma)) {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    if !matches!(self.next(), Some(Token::RParen)) {
                        return Err(DagError::Syntax(format!(
                            "Expected `)` after `{name}` args"
                        )));
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(DagError::Syntax("Expected closing `)`".into()));
                }
                Ok(inner)
            }
            _ => Err(DagError::Syntax("Unexpected end of expression".into())),
        }
    }
}

fn collect_deps(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Num(_) => {}
        Expr::Var(name) => {
            if name != "pi" {
                out.insert(name.clone());
            }
        }
        Expr::Neg(a) => collect_deps(a, out),
        Expr::Bin(_, a, b) => {
            collect_deps(a, out);
            collect_deps(b, out);
        }
        Expr::Call(_, args) => {
            for a in args {
                collect_deps(a, out);
            }
        }
    }
}

fn eval_ast(expr: &Expr, env: &HashMap<String, f64>) -> Result<f64, DagError> {
    match expr {
        Expr::Num(v) => Ok(*v),
        Expr::Var(name) => {
            if name == "pi" {
                return Ok(std::f64::consts::PI);
            }
            env.get(name)
                .copied()
                .ok_or_else(|| DagError::UnknownVar(name.clone()))
        }
        Expr::Neg(a) => Ok(-eval_ast(a, env)?),
        Expr::Bin(op, a, b) => {
            let x = eval_ast(a, env)?;
            let y = eval_ast(b, env)?;
            Ok(match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => {
                    if y.abs() < 1e-12 {
                        return Err(DagError::Syntax("Division by zero".into()));
                    }
                    x / y
                }
                BinOp::Gt => if x > y { 1.0 } else { 0.0 },
                BinOp::Lt => if x < y { 1.0 } else { 0.0 },
                BinOp::Eq => if (x - y).abs() < 1e-9 { 1.0 } else { 0.0 },
            })
        }
        Expr::Call(name, args) => {
            let vals: Vec<f64> = args
                .iter()
                .map(|a| eval_ast(a, env))
                .collect::<Result<_, _>>()?;
            match (name.as_str(), vals.as_slice()) {
                ("min", [a, b]) => Ok(a.min(*b)),
                ("max", [a, b]) => Ok(a.max(*b)),
                ("clamp", [v, lo, hi]) => Ok(v.clamp(*lo, *hi)),
                ("abs", [a]) => Ok(a.abs()),
                ("sqrt", [a]) => Ok(a.max(0.0).sqrt()),
                ("hypot", [a, b]) => Ok(a.hypot(*b)),
                ("sin", [a]) => Ok(a.to_radians().sin()),
                ("cos", [a]) => Ok(a.to_radians().cos()),
                ("tan", [a]) => Ok(a.to_radians().tan()),
                ("atan2", [y, x]) => Ok(y.atan2(*x).to_degrees()),
                ("if", [c, a, b]) => Ok(if c.abs() > 1e-9 { *a } else { *b }),
                _ => Err(DagError::Syntax(format!(
                    "Unknown or mismatched function `{name}` with {} args",
                    vals.len()
                ))),
            }
        }
    }
}

pub fn eval_single_expr(expr_str: &str, env: &HashMap<String, f64>) -> Result<f64, DagError> {
    let tokens = tokenize(expr_str)?;
    let mut parser = Parser { tokens, pos: 0 };
    let ast = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(DagError::Syntax("Unexpected trailing tokens".into()));
    }
    eval_ast(&ast, env)
}

#[derive(Debug, Clone)]
pub struct DagEvalResult {
    pub values: HashMap<String, f64>,
    pub errors: HashMap<String, String>,
    pub topo_order: Vec<String>,
}

/// Evaluates all ParametricVars in topological order (Kahn's algorithm) over base inputs.
/// Detects cycles cleanly and retains fallback values if an expression has errors.
pub fn evaluate_dag(
    base_inputs: &HashMap<String, f64>,
    vars: &[ParametricVar],
    prev_values: Option<&HashMap<String, f64>>,
) -> DagEvalResult {
    let mut values = base_inputs.clone();
    let mut errors = HashMap::new();
    let var_names: HashSet<String> = vars.iter().map(|v| v.name.clone()).collect();
    let mut parsed: HashMap<String, Expr> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut indegree: HashMap<String, usize> = HashMap::new();

    for v in vars {
        indegree.insert(v.name.clone(), 0);
        adj.entry(v.name.clone()).or_default();
    }

    for v in vars {
        match tokenize(&v.expr).and_then(|tokens| {
            let mut p = Parser { tokens, pos: 0 };
            let ast = p.parse_expr()?;
            if p.pos < p.tokens.len() {
                return Err(DagError::Syntax("Unexpected trailing tokens".into()));
            }
            Ok(ast)
        }) {
            Ok(ast) => {
                let mut deps = HashSet::new();
                collect_deps(&ast, &mut deps);
                for dep in deps {
                    if var_names.contains(&dep) {
                        adj.entry(dep).or_default().push(v.name.clone());
                        *indegree.entry(v.name.clone()).or_default() += 1;
                    }
                }
                parsed.insert(v.name.clone(), ast);
            }
            Err(e) => {
                errors.insert(v.name.clone(), e.to_string());
                if let Some(prev) = prev_values.and_then(|m| m.get(&v.name)) {
                    values.insert(v.name.clone(), *prev);
                }
            }
        }
    }

    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut topo_order = Vec::with_capacity(vars.len());

    while let Some(u) = queue.pop_front() {
        topo_order.push(u.clone());
        if let Some(neighbors) = adj.get(&u) {
            for w in neighbors {
                if let Some(d) = indegree.get_mut(w) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(w.clone());
                    }
                }
            }
        }
    }

    // Report any nodes stuck in a dependency cycle
    if topo_order.len() < vars.len() {
        for v in vars {
            if !topo_order.contains(&v.name) && !errors.contains_key(&v.name) {
                errors.insert(
                    v.name.clone(),
                    DagError::Cycle(vec![v.name.clone()]).to_string(),
                );
                if let Some(prev) = prev_values.and_then(|m| m.get(&v.name)) {
                    values.insert(v.name.clone(), *prev);
                }
            }
        }
    }

    for name in &topo_order {
        if let Some(ast) = parsed.get(name) {
            match eval_ast(ast, &values) {
                Ok(val) if val.is_finite() => {
                    values.insert(name.clone(), val);
                }
                Ok(_) => {
                    errors.insert(name.clone(), "Non-finite result".into());
                    if let Some(prev) = prev_values.and_then(|m| m.get(name)) {
                        values.insert(name.clone(), *prev);
                    }
                }
                Err(e) => {
                    errors.insert(name.clone(), e.to_string());
                    if let Some(prev) = prev_values.and_then(|m| m.get(name)) {
                        values.insert(name.clone(), *prev);
                    }
                }
            }
        }
    }

    DagEvalResult {
        values,
        errors,
        topo_order,
    }
}
