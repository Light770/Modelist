/**
 * Project-level actions shared by the app bar menus, the toolbar and keyboard shortcuts.
 * They only call the kernel and the store; no geometry happens here.
 */
import { closeDocument, getActiveDocumentId, kernel, openDocument, switchDocument } from './kernelClient';
import type { UiSnapshot } from './types';
import { openTextFileFromDisk, saveProjectToDisk } from './localFiles';
import { t, tCatalog } from './i18n';
import { useStudioStore } from '../store/studioStore';

/** Modifier-key label for shortcut hints ("⌘" on macOS, "Ctrl+" elsewhere). */
export const MOD_LABEL =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.userAgent) ? '⌘' : 'Ctrl+';

export async function saveProject(): Promise<void> {
  const st = useStudioStore.getState();
  if (!st.snapshot) return;
  try {
    const savedName = await saveProjectToDisk(st.snapshot.project_name, kernel.serializeProject());
    st.setStatusMessage(t('topbar.saved', { name: savedName }));
  } catch {
    // Save dialog cancelled
  }
}

/** Tool and dialog state belongs to one document; switching tabs starts clean. */
function showDocument(snapshot: UiSnapshot, addId?: string) {
  const st = useStudioStore.getState();
  if (st.seamCommand) st.closeSeamCommand();
  st.setActiveTool('select');
  const activeId = getActiveDocumentId();
  const docs = addId
    ? [...st.documents, { id: addId, name: snapshot.project_name }]
    : st.documents;
  useStudioStore.setState({
    activeWalkLinkId: null,
    walkFrame: null,
    editingOpId: null,
    panelPage: null,
    panelDismissedKey: null,
  });
  st.setDocuments(docs, activeId);
  st.setSnapshot(snapshot);
  st.requestFitView();
}

/** Adds a garment from the library to the current document, on top of the others. */
export function addGarment(templateId: string): void {
  const st = useStudioStore.getState();
  // The garment starts with the template name in the UI language (then it is user data).
  const snap = kernel.addComponent(templateId, tCatalog('template', `${templateId}.name`, ''));
  const added = snap.components.find((c) => c.id === snap.active_component_id);
  st.setSnapshot(snap);
  if (added) st.setStatusMessage(t('status.garmentAdded', { name: added.name, layer: added.layer }));
  st.requestFitView();
}

export function removeGarment(componentId: string): void {
  const st = useStudioStore.getState();
  st.setSnapshot(kernel.removeComponent(componentId));
  st.setStatusMessage(t('status.garmentRemoved'));
  st.requestFitView();
}

/** Opens a template in a new document tab, with nothing selected. */
export async function newFromTemplate(templateId: string): Promise<void> {
  const { id } = await openDocument(templateId);
  showDocument(kernel.setSelection(null, null), id);
}

export async function openProject(): Promise<void> {
  const file = await openTextFileFromDisk('.modelist.json,.modelist,.json');
  if (!file) return;
  const st = useStudioStore.getState();
  try {
    const previousId = getActiveDocumentId();
    const { id } = await openDocument('bodice_sleeve');
    try {
      kernel.loadProjectJson(file.text);
    } catch (e) {
      // Drop the new tab and go back to the document that was open.
      closeDocument(id);
      switchDocument(previousId);
      throw e;
    }
    showDocument(kernel.setSelection(null, null), id);
    st.setStatusMessage(t('topbar.opened', { name: file.name }));
  } catch (e) {
    st.setStatusMessage(t('topbar.openFailed', { error: (e as Error).message }));
  }
}

export function activateDocument(id: string): void {
  if (id === useStudioStore.getState().activeDocId) return;
  const snap = switchDocument(id);
  if (snap) showDocument(snap);
}

export function closeDocumentTab(id: string): void {
  const nextId = closeDocument(id);
  if (!nextId) return;
  const st = useStudioStore.getState();
  st.setDocuments(
    st.documents.filter((d) => d.id !== id),
    nextId,
  );
  const snap = switchDocument(nextId);
  if (snap) showDocument(snap);
}

/** Registers the first document after the kernel boots. */
export function registerInitialDocument(snapshot: UiSnapshot): UiSnapshot {
  const cleared = kernel.setSelection(null, null);
  useStudioStore.getState().setDocuments([{ id: getActiveDocumentId(), name: snapshot.project_name }], getActiveDocumentId());
  return cleared;
}

export function undo(): void {
  const next = kernel.undo();
  if (next) useStudioStore.getState().setSnapshot(next);
}

export function redo(): void {
  const next = kernel.redo();
  if (next) useStudioStore.getState().setSnapshot(next);
}

/** Inserts pieces from a DXF-AAMA or GarmentCode file into the open document. */
export async function importPattern(kind: 'dxf' | 'garmentcode'): Promise<void> {
  const file = await openTextFileFromDisk(kind === 'dxf' ? '.dxf' : '.json');
  if (!file) return;
  const st = useStudioStore.getState();
  try {
    const { message, snapshot } =
      kind === 'dxf' ? kernel.importDxfAama(file.text) : kernel.importGarmentCodeJson(file.text);
    st.setSnapshot(snapshot);
    st.requestFitView();
    st.setStatusMessage(message);
  } catch (e) {
    st.setStatusMessage(t('topbar.openFailed', { error: (e as Error).message }));
  }
}
