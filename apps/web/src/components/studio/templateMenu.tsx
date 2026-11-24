import React from 'react';
import { getTemplateCatalog } from '../../lib/templateCatalog';
import { useCatalogT, useT } from '../../lib/i18n';
import { addGarment } from '../../lib/projectActions';
import { useStudioStore } from '../../store/studioStore';
import { Menu, type MenuEntry } from './Menu';

type T = ReturnType<typeof useT>;
type CT = ReturnType<typeof useCatalogT>;

/** The template library as menu entries (blocks, then styles), each running `onPick`. */
export function templateMenuEntries(t: T, ct: CT, onPick: (templateId: string) => void): MenuEntry[] {
  const catalog = getTemplateCatalog();
  const items = (category: 'block' | 'style'): MenuEntry[] =>
    catalog
      .filter((tp) => tp.category === category)
      .map((tp) => ({
        id: `tpl-${tp.id}`,
        label: ct('template', `${tp.id}.name`, tp.name),
        title: ct('template', `${tp.id}.description`, tp.description),
        onSelect: () => onPick(tp.id),
      }));
  return [
    { kind: 'heading', id: 'h-blocks', label: t('topbar.blocks') },
    ...items('block'),
    { kind: 'heading', id: 'h-styles', label: t('topbar.styles') },
    ...items('style'),
  ];
}

/** The "add a garment to this document" menu, opened from the toolbar or the browser. */
export const GarmentPicker: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const anchor = useStudioStore((s) => s.garmentPicker);
  const close = useStudioStore((s) => s.openGarmentPicker);
  if (!anchor) return null;
  return (
    <Menu
      anchor={anchor}
      entries={templateMenuEntries(t, ct, (id) => addGarment(id))}
      triggerId="add-garment"
      onClose={() => close(null)}
    />
  );
};
