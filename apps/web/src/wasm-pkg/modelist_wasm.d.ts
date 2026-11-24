/* tslint:disable */
/* eslint-disable */

export class ModelistSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Adds a garment drafted from a template, to the right of the others on the canvas and
     * one layer above them in the 3D fit. It becomes the active garment. `name` (the template
     * name in the UI language) is optional. Returns its id.
     */
    add_component(template_id: string, name: string): string;
    add_drill_hole(piece_id: string, x: number, y: number, label: string): void;
    add_notch(piece_id: string, edge_idx: number, param_t: number, style: string, count: number, depth_mm: number): void;
    add_rect_piece(name: string, x0: number, y0: number, x1: number, y1: number): void;
    auto_detect_seams(): number;
    clear_seam_walk(): void;
    compute_marker_plan_json(config_json: string): string;
    compute_seam_walk(link_id: string, progress_u: number): string;
    convert_edge_curve(piece_id: string, edge_idx: number, to_cubic: boolean): void;
    create_seam_binding(label: string, piece_a_id: string, edge_a_id: string, piece_b_id: string, edge_b_id: string, driver: string, ease_json: string): void;
    delete_seam_binding(binding_id: string): void;
    delete_variable(name: string): void;
    dispose_sim3d(): void;
    duplicate_piece(piece_id: string): void;
    export_aama_rul(): string;
    export_dxf_aama(): string;
    export_marker_svg(config_json: string): string;
    export_pom_csv(): string;
    export_svg_mm(include_nest: boolean): string;
    get_handle_points(): Float32Array;
    get_pdf_layout_json(preset: string, unfold_on_fold: boolean): string;
    get_sim3d_avatar_indices(): Uint32Array;
    get_sim3d_avatar_normals(): Float32Array;
    get_sim3d_avatar_positions(): Float32Array;
    get_sim3d_avatar_torso_mask(): Float32Array;
    get_sim3d_cloth_clearance(): Float32Array;
    /**
     * Per-vertex cloth colour (rgb 0..1) from each piece role's fabric.
     */
    get_sim3d_cloth_colors(): Float32Array;
    get_sim3d_cloth_indices(): Uint32Array;
    get_sim3d_cloth_normals(): Float32Array;
    get_sim3d_cloth_positions(): Float32Array;
    get_sim3d_cloth_strain(): Float32Array;
    get_sim3d_cloth_tension(): Float32Array;
    /**
     * Drape-quality metrics (poke-through, creases, self-intersections) as JSON. A few ms:
     * poll it occasionally, not every frame.
     */
    get_sim3d_quality_json(): string;
    get_stroke_coords(): Float32Array;
    get_stroke_meta(): Uint32Array;
    get_ui_snapshot_json(): string;
    hit_test(wx: number, wy: number, world_tol_mm: number): string;
    import_dxf_aama(dxf_text: string): string;
    import_garmentcode_json(json_text: string): string;
    init_sim3d(pose: string, body_form: string): string;
    load_project_json(json: string): void;
    load_template(template_id: string): void;
    mirror_piece(piece_id: string): void;
    /**
     * Moves a garment one place outward (worn over the next garment) or inward, swapping
     * layers with its neighbour; layers are then renumbered 0, 1, 2... in wearing order.
     */
    move_component_layer(component_id: string, outward: boolean): void;
    move_handle(piece_id: string, edge_idx: number, which: number, total_dx: number, total_dy: number, lock_g1: boolean, commit: boolean): void;
    move_piece(piece_id: string, total_dx: number, total_dy: number, commit: boolean): void;
    move_vertex(piece_id: string, edge_idx: number, total_dx: number, total_dy: number, commit: boolean): void;
    constructor(template_id: string);
    pivot_dart(piece_id: string, dart_id: string, target_edge_idx: number, target_t: number, angle_deg: number): void;
    query_snap(wx: number, wy: number, world_tol_mm: number): string;
    redo(): boolean;
    /**
     * Removes a garment (the last one cannot be removed).
     */
    remove_component(component_id: string): void;
    remove_op(op_id: string): void;
    rename_component(component_id: string, name: string): void;
    reorder_op(from_idx: number, to_idx: number): boolean;
    /**
     * Restores the shell fabric's physical properties from its preset (keeps the colour).
     */
    reset_fabric_to_preset(): void;
    reset_sim3d(pose: string, body_form: string): string;
    serialize_project_json(): string;
    /**
     * Makes a garment the one edits, parameters, fabrics and the timeline apply to.
     */
    set_active_component(component_id: string): void;
    set_active_size(size: string): void;
    /**
     * Garment layer in the 3D fit (0 = next to the body). Re-dresses the 3D fit when open.
     */
    set_component_layer(component_id: string, layer: number): void;
    /**
     * Shows or hides a garment in the 3D fit only. A hidden garment is still worn, so hiding
     * an outer layer reveals what is under it without re-draping anything.
     */
    set_component_visible_3d(component_id: string, visible: boolean): void;
    /**
     * Shows or hides a garment everywhere (canvas, exports, 3D fit).
     */
    set_component_visible(component_id: string, visible: boolean): void;
    set_ease(key: string, val: number): void;
    set_edge_length(piece_id: string, edge_idx: number, target_len_mm: number): void;
    set_edge_sa(piece_id: string, edge_idx: number, width_mm: number, corner_rule_json: string): void;
    set_fabric_color(color_hex: string): void;
    /**
     * Surface finish for the 3D render: plain, twill, satin, jersey, rib, sheer, terry.
     */
    set_fabric_finish(finish: string): void;
    set_fabric_preset(preset_id: string): void;
    /**
     * Sets one physical property of the shell fabric (key from `fabric_property_specs_json`).
     * Values are clamped to the property's valid range. Returns false for unknown keys.
     */
    set_fabric_property(key: string, value: number): boolean;
    set_grade_mode(mode: string): void;
    set_grain_rotation(piece_id: string, mode: string): void;
    set_measurement(key: string, val: number): void;
    set_project_name(name: string): void;
    set_role_fabric_color(role: string, color_hex: string): void;
    /**
     * Assigns a preset to a piece role (e.g. "rib"); an empty preset id means "same as shell".
     */
    set_role_fabric_preset(role: string, preset_id: string): void;
    /**
     * Sets one physical property of a role's fabric (the role must have its own fabric).
     */
    set_role_fabric_property(role: string, key: string, value: number): boolean;
    set_rollback_index(index: number): void;
    set_seam_draft_preview(edges_a_json: string, edges_b_json: string): void;
    set_selected_binding(binding_id?: string | null): void;
    set_selection(piece_id?: string | null, edge_idx?: number | null): void;
    set_show_mirror_unfold(show: boolean): void;
    set_stack_anchor(anchor: string): void;
    set_variable(name: string, expr: string, description: string): void;
    set_visible_sizes(sizes_json: string): void;
    slash_and_spread(piece_id: string, start_edge_idx: number, end_edge_idx: number, slash_count: number, spread_mm: number, hinge_mode: string): void;
    split_edge_point(piece_id: string, edge_idx: number, param_t: number): void;
    split_style_line(piece_id: string, edge_a_idx: number, t_a: number, edge_b_idx: number, t_b: number, label_a: string, label_b: string): void;
    step_sim3d(steps: number): string;
    toggle_piece_visibility(piece_id: string): void;
    toggle_suppress_op(op_id: string): void;
    true_corner(piece_id: string, edge_idx: number, square_90: boolean): void;
    undo(): boolean;
    update_op_json(op_id: string, patch_json: string): void;
    update_seam_binding(binding_id: string, driver: string, ease_json: string, seam_type: string, auto_partner_notches: boolean): void;
    /**
     * Creates or replaces a full multi-edge SeamBinding (used by the Fusion 360 Seam Command Dialog).
     */
    upsert_seam_binding(binding_id: string | null | undefined, label: string, edges_a_json: string, edges_b_json: string, driver: string, ease_json: string, seam_type: string, auto_partner_notches: boolean, sync_recommended_sa: boolean): string;
}

/**
 * Editable fabric properties (keys, units, ranges, help) for the fabric editor.
 */
export function fabric_property_specs_json(): string;

/**
 * Starter library catalog (blocks + styles) as JSON, so the UI never hard-codes template ids.
 */
export function template_catalog_json(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_modelistsession_free: (a: number, b: number) => void;
    readonly fabric_property_specs_json: () => [number, number];
    readonly modelistsession_add_component: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly modelistsession_add_drill_hole: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly modelistsession_add_notch: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly modelistsession_add_rect_piece: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly modelistsession_auto_detect_seams: (a: number) => number;
    readonly modelistsession_clear_seam_walk: (a: number) => void;
    readonly modelistsession_compute_marker_plan_json: (a: number, b: number, c: number) => [number, number];
    readonly modelistsession_compute_seam_walk: (a: number, b: number, c: number, d: number) => [number, number];
    readonly modelistsession_convert_edge_curve: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_create_seam_binding: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => void;
    readonly modelistsession_delete_seam_binding: (a: number, b: number, c: number) => void;
    readonly modelistsession_delete_variable: (a: number, b: number, c: number) => void;
    readonly modelistsession_dispose_sim3d: (a: number) => void;
    readonly modelistsession_duplicate_piece: (a: number, b: number, c: number) => void;
    readonly modelistsession_export_aama_rul: (a: number) => [number, number];
    readonly modelistsession_export_dxf_aama: (a: number) => [number, number];
    readonly modelistsession_export_marker_svg: (a: number, b: number, c: number) => [number, number];
    readonly modelistsession_export_pom_csv: (a: number) => [number, number];
    readonly modelistsession_export_svg_mm: (a: number, b: number) => [number, number];
    readonly modelistsession_get_handle_points: (a: number) => any;
    readonly modelistsession_get_pdf_layout_json: (a: number, b: number, c: number, d: number) => [number, number];
    readonly modelistsession_get_sim3d_avatar_indices: (a: number) => any;
    readonly modelistsession_get_sim3d_avatar_normals: (a: number) => any;
    readonly modelistsession_get_sim3d_avatar_positions: (a: number) => any;
    readonly modelistsession_get_sim3d_avatar_torso_mask: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_clearance: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_colors: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_indices: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_normals: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_positions: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_strain: (a: number) => any;
    readonly modelistsession_get_sim3d_cloth_tension: (a: number) => any;
    readonly modelistsession_get_sim3d_quality_json: (a: number) => [number, number];
    readonly modelistsession_get_stroke_coords: (a: number) => any;
    readonly modelistsession_get_stroke_meta: (a: number) => any;
    readonly modelistsession_get_ui_snapshot_json: (a: number) => [number, number];
    readonly modelistsession_hit_test: (a: number, b: number, c: number, d: number) => [number, number];
    readonly modelistsession_import_dxf_aama: (a: number, b: number, c: number) => [number, number, number, number];
    readonly modelistsession_import_garmentcode_json: (a: number, b: number, c: number) => [number, number, number, number];
    readonly modelistsession_init_sim3d: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly modelistsession_load_project_json: (a: number, b: number, c: number) => [number, number];
    readonly modelistsession_load_template: (a: number, b: number, c: number) => void;
    readonly modelistsession_mirror_piece: (a: number, b: number, c: number) => void;
    readonly modelistsession_move_component_layer: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_move_handle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly modelistsession_move_piece: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly modelistsession_move_vertex: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly modelistsession_new: (a: number, b: number) => number;
    readonly modelistsession_pivot_dart: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly modelistsession_query_snap: (a: number, b: number, c: number, d: number) => [number, number];
    readonly modelistsession_redo: (a: number) => number;
    readonly modelistsession_remove_component: (a: number, b: number, c: number) => void;
    readonly modelistsession_remove_op: (a: number, b: number, c: number) => void;
    readonly modelistsession_rename_component: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_reorder_op: (a: number, b: number, c: number) => number;
    readonly modelistsession_reset_fabric_to_preset: (a: number) => void;
    readonly modelistsession_reset_sim3d: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly modelistsession_serialize_project_json: (a: number) => [number, number];
    readonly modelistsession_set_active_component: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_active_size: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_component_layer: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_component_visible: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_component_visible_3d: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_ease: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_edge_length: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_set_edge_sa: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly modelistsession_set_fabric_color: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_fabric_finish: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_fabric_preset: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_fabric_property: (a: number, b: number, c: number, d: number) => number;
    readonly modelistsession_set_grade_mode: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_grain_rotation: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_set_measurement: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_project_name: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_role_fabric_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_set_role_fabric_preset: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_set_role_fabric_property: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly modelistsession_set_rollback_index: (a: number, b: number) => void;
    readonly modelistsession_set_seam_draft_preview: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_set_selected_binding: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_selection: (a: number, b: number, c: number, d: number) => void;
    readonly modelistsession_set_show_mirror_unfold: (a: number, b: number) => void;
    readonly modelistsession_set_stack_anchor: (a: number, b: number, c: number) => void;
    readonly modelistsession_set_variable: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly modelistsession_set_visible_sizes: (a: number, b: number, c: number) => void;
    readonly modelistsession_slash_and_spread: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly modelistsession_split_edge_point: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_split_style_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly modelistsession_step_sim3d: (a: number, b: number) => [number, number];
    readonly modelistsession_toggle_piece_visibility: (a: number, b: number, c: number) => void;
    readonly modelistsession_toggle_suppress_op: (a: number, b: number, c: number) => void;
    readonly modelistsession_true_corner: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly modelistsession_undo: (a: number) => number;
    readonly modelistsession_update_op_json: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly modelistsession_update_seam_binding: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly modelistsession_upsert_seam_binding: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number) => [number, number, number, number];
    readonly template_catalog_json: () => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
