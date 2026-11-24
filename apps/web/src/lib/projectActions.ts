/**
 * Project-level actions shared by the app bar menus, the toolbar and keyboard shortcuts.
 * They only call the kernel and the store; no geometry happens here.
 */
import { closeDocument, getActiveDocumentId, kernel, openDocument, switchDocument } from './kernelClient';
import type { UiSnapshot } from './types';
import { openBinaryFileFromDisk, openTextFileFromDisk, saveProjectJsonToDisk, saveProjectToDisk } from './localFiles';
import { t, tCatalog } from './i18n';
import { deleteSavedGarment, getSavedGarment, saveGarment } from './garmentLibrary';
import { useStudioStore } from '../store/studioStore';

/** Modifier-key label for shortcut hints ("⌘" on macOS, "Ctrl+" elsewhere). */
export const MOD_LABEL =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.userAgent) ? '⌘' : 'Ctrl+';

/** A small PNG of the 2D canvas for the file's preview (empty when there is no canvas). */
export async function canvasThumbnail(maxSize = 320): Promise<Uint8Array> {
  const src = document.querySelector<HTMLCanvasElement>('canvas[data-viewport2d]');
  if (!src || !src.width || !src.height) return new Uint8Array();
  const k = Math.min(1, maxSize / Math.max(src.width, src.height));
  const c = document.createElement('canvas');
  c.width = Math.max(1, Math.round(src.width * k));
  c.height = Math.max(1, Math.round(src.height * k));
  c.getContext('2d')?.drawImage(src, 0, 0, c.width, c.height);
  const blob = await new Promise<Blob | null>((r) => c.toBlob(r, 'image/png'));
  return blob ? new Uint8Array(await blob.arrayBuffer()) : new Uint8Array();
}

/** UI state saved in the file's `session.json`, restored when it is opened. */
interface SavedSession {
  workspace_mode?: string;
  toolbar_tab?: string;
  unit?: string;
}

function currentSession(): string {
  const st = useStudioStore.getState();
  const s: SavedSession = { workspace_mode: st.workspaceMode, toolbar_tab: st.toolbarTab, unit: st.unit };
  return JSON.stringify(s);
}

function restoreSession(json: string) {
  if (!json) return;
  try {
    const s = JSON.parse(json) as SavedSession;
    const st = useStudioStore.getState();
    if (s.workspace_mode === '2d' || s.workspace_mode === '3d' || s.workspace_mode === 'split') st.setWorkspaceMode(s.workspace_mode);
    if (s.unit === 'mm' || s.unit === 'cm' || s.unit === 'in') st.setUnit(s.unit);
    if (s.toolbar_tab) st.setToolbarTab(s.toolbar_tab as never);
  } catch {
    // UI state is optional.
  }
}

/** Saves the document as a `.modelist` file (ZIP container with a preview). */
export async function saveProject(): Promise<void> {
  const st = useStudioStore.getState();
  if (!st.snapshot) return;
  try {
    const bytes = kernel.saveProjectContainer(currentSession(), await canvasThumbnail());
    const savedName = await saveProjectToDisk(st.snapshot.project_name, bytes);
    st.setStatusMessage(t('topbar.saved', { name: savedName }));
  } catch (e) {
    if ((e as Error)?.name !== 'AbortError') st.setStatusMessage(t('topbar.saveFailed', { error: (e as Error).message }));
  }
}

/** Saves the whole document as one JSON file (debugging, version control). */
export async function exportProjectJson(): Promise<void> {
  const st = useStudioStore.getState();
  if (!st.snapshot) return;
  try {
    const savedName = await saveProjectJsonToDisk(st.snapshot.project_name, kernel.serializeProject());
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

/** Adds a copy of a garment (with its history) beside the others. */
export function duplicateGarment(componentId: string): void {
  const st = useStudioStore.getState();
  st.setSnapshot(kernel.duplicateComponent(componentId));
  st.requestFitView();
}

/** Saves a garment to the local garment library (with a preview of the canvas). */
export async function saveGarmentToLibrary(componentId: string): Promise<void> {
  const st = useStudioStore.getState();
  const comp = st.snapshot?.components.find((c) => c.id === componentId);
  if (!comp) return;
  try {
    const thumbnail = await canvasThumbnail(240);
    const bytes = kernel.exportComponentFile(componentId, thumbnail);
    await saveGarment({ name: comp.name, template_id: comp.template_id, thumbnail, bytes });
    st.bumpLibrary();
    st.setStatusMessage(t('status.garmentSaved', { name: comp.name }));
  } catch (e) {
    st.setStatusMessage(t('topbar.saveFailed', { error: (e as Error).message }));
  }
}

/** Inserts a garment from the library into the open document, refitted to its body. */
export async function insertSavedGarment(entryId: string): Promise<void> {
  const st = useStudioStore.getState();
  const entry = await getSavedGarment(entryId);
  if (!entry) return;
  try {
    st.setSnapshot(kernel.insertComponentFile(entry.bytes));
    st.setStatusMessage(t('status.garmentInserted', { name: entry.name }));
    st.requestFitView();
  } catch (e) {
    st.setStatusMessage(t('topbar.openFailed', { error: (e as Error).message }));
  }
}

/** Writes a saved garment, or a garment of the document, to a `.modelist` file. */
export async function exportGarmentFile(source: { entryId?: string; componentId?: string }): Promise<void> {
  const st = useStudioStore.getState();
  try {
    if (source.entryId) {
      const entry = await getSavedGarment(source.entryId);
      if (entry) await saveProjectToDisk(entry.name, entry.bytes);
    } else if (source.componentId) {
      const comp = st.snapshot?.components.find((c) => c.id === source.componentId);
      const bytes = kernel.exportComponentFile(source.componentId, await canvasThumbnail(240));
      await saveProjectToDisk(comp?.name ?? 'garment', bytes);
    }
  } catch {
    // Save dialog cancelled
  }
}

/** Adds the garments of a `.modelist` file to the open document. */
export async function importGarmentFile(): Promise<void> {
  const file = await openBinaryFileFromDisk('.modelist,.json');
  if (!file) return;
  const st = useStudioStore.getState();
  try {
    st.setSnapshot(kernel.insertComponentFile(file.bytes));
    st.setStatusMessage(t('status.garmentInserted', { name: file.name }));
    st.requestFitView();
  } catch (e) {
    st.setStatusMessage(t('topbar.openFailed', { error: (e as Error).message }));
  }
}

export async function deleteLibraryGarment(entryId: string): Promise<void> {
  await deleteSavedGarment(entryId);
  useStudioStore.getState().bumpLibrary();
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
  const file = await openBinaryFileFromDisk('.modelist,.json');
  if (!file) return;
  const st = useStudioStore.getState();
  try {
    const previousId = getActiveDocumentId();
    const { id } = await openDocument('bodice_sleeve');
    let session = '';
    try {
      session = kernel.loadProjectFile(file.bytes).session;
    } catch (e) {
      // Drop the new tab and go back to the document that was open.
      closeDocument(id);
      switchDocument(previousId);
      throw e;
    }
    showDocument(kernel.setSelection(null, null), id);
    restoreSession(session);
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
