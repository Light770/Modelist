import React, { useEffect, useState } from 'react';
import { BookMarked, FileInput } from 'lucide-react';
import { listSavedGarments, type SavedGarment } from '../../lib/garmentLibrary';
import { getTemplateCatalog } from '../../lib/templateCatalog';
import { useCatalogT, useT } from '../../lib/i18n';
import { addGarment, importGarmentFile, insertSavedGarment } from '../../lib/projectActions';
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

/** The "add a garment to this document" menu, opened from the toolbar or the browser: the
 *  user's saved garments first, then the template library, then importing a garment file. */
export const GarmentPicker: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const anchor = useStudioStore((s) => s.garmentPicker);
  const close = useStudioStore((s) => s.openGarmentPicker);
  const libraryVersion = useStudioStore((s) => s.libraryVersion);
  const [saved, setSaved] = useState<SavedGarment[]>([]);
  useEffect(() => {
    if (anchor) void listSavedGarments().then(setSaved);
  }, [anchor, libraryVersion]);
  if (!anchor) return null;
  const mine: MenuEntry[] = saved.length
    ? [
        { kind: 'heading', id: 'h-mine', label: t('library.myGarments') },
        ...saved.map(
          (g): MenuEntry => ({
            id: `saved-${g.id}`,
            label: g.name,
            icon: BookMarked,
            title: t('library.savedOn', { date: new Date(g.saved_at).toLocaleDateString() }),
            onSelect: () => void insertSavedGarment(g.id),
          }),
        ),
        { kind: 'separator', id: 's-mine' },
      ]
    : [];
  return (
    <Menu
      anchor={anchor}
      entries={[
        ...mine,
        ...templateMenuEntries(t, ct, (id) => addGarment(id)),
        { kind: 'separator', id: 's-import' },
        { id: 'import-garment', label: t('library.importFile'), icon: FileInput, onSelect: () => void importGarmentFile() },
      ]}
      triggerId="add-garment"
      onClose={() => close(null)}
    />
  );
};
