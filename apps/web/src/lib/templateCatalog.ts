import { template_catalog_json } from '../wasm-pkg/modelist_wasm.js';

export type TemplateCategory = 'block' | 'style';
export type ViewRegion = 'torso' | 'hips' | 'legs';

/** Mirrors `modelist_core::library::TemplateInfo`, serialized by `template_catalog_json()`. */
export interface TemplateInfo {
  id: string;
  name: string;
  category: TemplateCategory;
  garment: string;
  silhouette: string;
  description: string;
  tags: string[];
  measurement_keys: string[];
  ease_keys: string[];
  view_region: ViewRegion;
}

let cache: TemplateInfo[] | null = null;

/** The kernel's template catalog. Only call after the WASM kernel has been initialised. */
export function getTemplateCatalog(): TemplateInfo[] {
  if (!cache) {
    try {
      cache = JSON.parse(template_catalog_json()) as TemplateInfo[];
    } catch {
      return [];
    }
  }
  return cache;
}

export function findTemplate(id: string | undefined): TemplateInfo | undefined {
  return getTemplateCatalog().find((t) => t.id === id);
}
