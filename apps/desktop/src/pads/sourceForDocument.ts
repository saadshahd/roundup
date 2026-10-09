import type { Node as DocumentNode } from "@milkdown/kit/prose/model";

export const sourceForDocument = (
  originalSource: string,
  originalDocument: DocumentNode,
  currentDocument: DocumentNode,
  serialize: () => string,
) => currentDocument.eq(originalDocument) ? originalSource : serialize();
