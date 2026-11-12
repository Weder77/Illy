// Éditeurs ouverts dans cette fenêtre : à enregistrer avant de la fermer (ou avant une mise à jour).
const openComposers = new Set<() => Promise<void>>();

export function registerComposer(save: () => Promise<void>): () => void {
  openComposers.add(save);
  return () => openComposers.delete(save);
}

export async function saveOpenComposers(): Promise<void> {
  await Promise.all([...openComposers].map((save) => save().catch(() => undefined)));
}
