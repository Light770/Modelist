import {
  PDFDocument,
  StandardFonts,
  clip,
  endPath,
  popGraphicsState,
  pushGraphicsState,
  rectangle,
  rgb,
} from 'pdf-lib';
import type { PdfLayerPath, PdfLayoutPlan } from './types';

const MM_TO_PT = 72 / 25.4;

export interface PdfExportOptions {
  preset: 'a4_tiled' | 'letter_tiled' | 'a0' | 'projector';
  unfoldOnFold: boolean;
  darkProjector: boolean;
  showGrid: boolean;
  projectName: string;
}

const SIZE_COLORS = [
  rgb(0.11, 0.14, 0.19),
  rgb(0.12, 0.37, 0.92),
  rgb(0.05, 0.56, 0.42),
  rgb(0.84, 0.46, 0.06),
  rgb(0.49, 0.23, 0.93),
  rgb(0.86, 0.15, 0.15),
];

function sizeColor(sizeName: string, darkBg: boolean) {
  if (darkBg) return rgb(0.97, 0.98, 1.0);
  const idx = ['XS', 'S', 'M', 'L', 'XL', 'XXL'].indexOf(sizeName);
  return idx >= 0 ? SIZE_COLORS[idx % SIZE_COLORS.length] : SIZE_COLORS[0];
}

function drawPathOnPage(
  page: ReturnType<PDFDocument['addPage']>,
  p: PdfLayerPath,
  originXMm: number,
  originYMm: number,
  offsetPtX: number,
  offsetPtY: number,
  scaleFactor: number,
  darkBg: boolean,
  thickMode: boolean,
) {
  if (p.pts.length < 2) return;
  const toX = (mmX: number) => offsetPtX + (mmX - originXMm) * MM_TO_PT * scaleFactor;
  const toY = (mmY: number) => offsetPtY + (mmY - originYMm) * MM_TO_PT * scaleFactor;

  let color = sizeColor(p.size_name, darkBg);
  let thickness = thickMode ? 2.4 : 1.1;
  let dashArray: number[] | undefined;

  if (p.layer_kind === 'cut') {
    color = darkBg ? rgb(0.98, 0.75, 0.2) : rgb(0.78, 0.44, 0.07);
    thickness = thickMode ? 2.2 : 0.95;
    dashArray = [5, 3];
  } else if (p.layer_kind === 'grain') {
    color = darkBg ? rgb(0.42, 0.72, 1) : rgb(0.12, 0.37, 0.92);
    thickness = thickMode ? 1.5 : 0.75;
  } else if (p.layer_kind === 'notch') {
    color = darkBg ? rgb(1, 0.45, 0.45) : rgb(0.82, 0.18, 0.18);
    thickness = thickMode ? 2.2 : 1.1;
  } else if (p.layer_kind === 'dart') {
    color = darkBg ? rgb(0.7, 0.76, 0.84) : rgb(0.42, 0.45, 0.52);
    thickness = thickMode ? 1.4 : 0.7;
    dashArray = [3, 3];
  }

  const pts = p.pts;
  for (let i = 0; i < pts.length - 1; i++) {
    page.drawLine({
      start: { x: toX(pts[i].x), y: toY(pts[i].y) },
      end: { x: toX(pts[i + 1].x), y: toY(pts[i + 1].y) },
      thickness,
      color,
      dashArray,
    });
  }
  if (p.closed && pts.length > 2) {
    page.drawLine({
      start: { x: toX(pts[pts.length - 1].x), y: toY(pts[pts.length - 1].y) },
      end: { x: toX(pts[0].x), y: toY(pts[0].y) },
      thickness,
      color,
      dashArray,
    });
  }
}

function drawCalibrationSquare(
  page: ReturnType<PDFDocument['addPage']>,
  font: Awaited<ReturnType<PDFDocument['embedFont']>>,
  xPt: number,
  yPt: number,
  darkBg: boolean,
) {
  const sq = 100 * MM_TO_PT;
  const ink = darkBg ? rgb(0.95, 0.95, 0.95) : rgb(0.15, 0.18, 0.24);
  page.drawRectangle({
    x: xPt,
    y: yPt,
    width: sq,
    height: sq,
    borderColor: ink,
    borderWidth: 1.2,
  });
  page.drawText('100 mm x 100 mm', {
    x: xPt + 12,
    y: yPt + sq * 0.52,
    size: 10,
    font,
    color: ink,
  });
  page.drawText('CALIBRATION TEST SQUARE', {
    x: xPt + 12,
    y: yPt + sq * 0.36,
    size: 7.5,
    font,
    color: ink,
  });
}

export async function generatePdfBytes(
  plan: PdfLayoutPlan,
  opts: PdfExportOptions,
): Promise<Uint8Array> {
  const doc = await PDFDocument.create();
  doc.setTitle(`${opts.projectName} (${opts.preset})`);
  doc.setCreator('Modelist Patternmaking CAD');

  const font = await doc.embedFont(StandardFonts.Helvetica);
  const fontBold = await doc.embedFont(StandardFonts.HelveticaBold);
  const isProjector = opts.preset === 'projector';
  const darkBg = isProjector && opts.darkProjector;

  // Tiled prints include a clean Assembly Overview Map on Page 1
  if (opts.preset === 'a4_tiled' || opts.preset === 'letter_tiled') {
    const mapPage = doc.addPage([plan.page_width_mm * MM_TO_PT, plan.page_height_mm * MM_TO_PT]);
    const pw = plan.page_width_mm * MM_TO_PT;
    const ph = plan.page_height_mm * MM_TO_PT;
    mapPage.drawText(`${opts.projectName} — Tile Assembly Map`, {
      x: 36,
      y: ph - 48,
      size: 16,
      font: fontBold,
      color: rgb(0.1, 0.12, 0.16),
    });
    mapPage.drawText(`Total non-empty sheets: ${plan.tiles.length} · Print at 100% Actual Size`, {
      x: 36,
      y: ph - 66,
      size: 10,
      font,
      color: rgb(0.38, 0.41, 0.47),
    });

    const spanX = Math.max(100, plan.bounds_max_x - plan.bounds_min_x + 40);
    const spanY = Math.max(100, plan.bounds_max_y - plan.bounds_min_y + 40);
    const availW = pw - 72;
    const availH = ph - 140;
    const mapScale = Math.min(availW / (spanX * MM_TO_PT), availH / (spanY * MM_TO_PT));
    const mapOffX = 36;
    const mapOffY = 48;

    for (const p of plan.paths) {
      drawPathOnPage(
        mapPage,
        p,
        plan.bounds_min_x - 20,
        plan.bounds_min_y - 20,
        mapOffX,
        mapOffY,
        mapScale,
        false,
        false,
      );
    }

    for (const tile of plan.tiles) {
      const rx = mapOffX + (tile.x_min - (plan.bounds_min_x - 20)) * MM_TO_PT * mapScale;
      const ry = mapOffY + (tile.y_min - (plan.bounds_min_y - 20)) * MM_TO_PT * mapScale;
      const rw = (tile.x_max - tile.x_min) * MM_TO_PT * mapScale;
      const rh = (tile.y_max - tile.y_min) * MM_TO_PT * mapScale;
      mapPage.drawRectangle({
        x: rx,
        y: ry,
        width: rw,
        height: rh,
        borderColor: rgb(0.12, 0.37, 0.92),
        borderWidth: 0.7,
      });
      mapPage.drawText(`R${tile.row + 1}-C${tile.col + 1}`, {
        x: rx + 4,
        y: ry + rh - 11,
        size: 7.5,
        font: fontBold,
        color: rgb(0.12, 0.37, 0.92),
      });
    }
  }

  for (let ti = 0; ti < plan.tiles.length; ti++) {
    const tile = plan.tiles[ti];
    const pageW = plan.page_width_mm * MM_TO_PT;
    const pageH = plan.page_height_mm * MM_TO_PT;
    const marginPt = plan.margin_mm * MM_TO_PT;
    const page = doc.addPage([pageW, pageH]);

    if (darkBg) {
      page.drawRectangle({
        x: 0,
        y: 0,
        width: pageW,
        height: pageH,
        color: rgb(0.06, 0.07, 0.09),
      });
    }

    const innerW = pageW - 2 * marginPt;
    const innerH = pageH - 2 * marginPt;

    // Optional Projector 50 mm Calibration Grid
    if (isProjector && opts.showGrid) {
      const gridColor = darkBg ? rgb(0.16, 0.2, 0.27) : rgb(0.86, 0.88, 0.92);
      const stepMm = 50;
      const xStart = Math.ceil(tile.x_min / stepMm) * stepMm;
      for (let gx = xStart; gx <= tile.x_max; gx += stepMm) {
        const px = (gx - tile.x_min) * MM_TO_PT + marginPt;
        page.drawLine({
          start: { x: px, y: 0 },
          end: { x: px, y: pageH },
          thickness: 0.5,
          color: gridColor,
        });
      }
      const yStart = Math.ceil(tile.y_min / stepMm) * stepMm;
      for (let gy = yStart; gy <= tile.y_max; gy += stepMm) {
        const py = (gy - tile.y_min) * MM_TO_PT + marginPt;
        page.drawLine({
          start: { x: 0, y: py },
          end: { x: pageW, y: py },
          thickness: 0.5,
          color: gridColor,
        });
      }
    }

    // Clip pattern strokes cleanly inside tile border
    if (!isProjector && plan.tiles.length > 1) {
      page.pushOperators(
        pushGraphicsState(),
        rectangle(marginPt, marginPt, innerW, innerH),
        clip(),
        endPath(),
      );
    }

    for (const p of plan.paths) {
      drawPathOnPage(
        page,
        p,
        tile.x_min,
        tile.y_min,
        isProjector ? 0 : marginPt,
        isProjector ? 0 : marginPt,
        1.0,
        darkBg,
        isProjector,
      );
    }

    for (const lbl of plan.labels) {
      const lx = (isProjector ? 0 : marginPt) + (lbl.pos.x - tile.x_min) * MM_TO_PT;
      const ly = (isProjector ? 0 : marginPt) + (lbl.pos.y - tile.y_min) * MM_TO_PT;
      if (lx >= 0 && lx <= pageW && ly >= 0 && ly <= pageH) {
        page.drawText(lbl.text, {
          x: lx - 36,
          y: ly,
          size: isProjector ? 14 : 9,
          font: fontBold,
          color: darkBg ? rgb(0.95, 0.96, 0.99) : rgb(0.1, 0.12, 0.16),
        });
      }
    }

    if (!isProjector && plan.tiles.length > 1) {
      page.pushOperators(popGraphicsState());
      const frameColor = rgb(0.55, 0.58, 0.65);
      page.drawRectangle({
        x: marginPt,
        y: marginPt,
        width: innerW,
        height: innerH,
        borderColor: frameColor,
        borderWidth: 0.6,
        borderDashArray: [4, 4],
      });
      // Registration crosses at corners
      for (const [cx, cy] of [
        [marginPt, marginPt],
        [marginPt + innerW, marginPt],
        [marginPt, marginPt + innerH],
        [marginPt + innerW, marginPt + innerH],
      ]) {
        page.drawLine({ start: { x: cx - 6, y: cy }, end: { x: cx + 6, y: cy }, thickness: 0.7, color: frameColor });
        page.drawLine({ start: { x: cx, y: cy - 6 }, end: { x: cx, y: cy + 6 }, thickness: 0.7, color: frameColor });
      }
      page.drawText(`Tile R${tile.row + 1}-C${tile.col + 1} · ${opts.projectName} · Scale 100%`, {
        x: marginPt + 4,
        y: marginPt - 14,
        size: 8,
        font,
        color: rgb(0.35, 0.38, 0.45),
      });
    }

    if (ti === 0) {
      drawCalibrationSquare(
        page,
        fontBold,
        isProjector ? 60 : marginPt + 14,
        isProjector ? 60 : marginPt + 14,
        darkBg,
      );
    }
  }

  return doc.save();
}
