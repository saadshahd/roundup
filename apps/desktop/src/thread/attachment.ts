export type Attachment = { id: number; name: string; size: number; url: string };

const isImage = (file: File): boolean => file.type.startsWith("image/");

/** `812 B`, `12 KB`, `3.4 MB`: the size an Attachment shows beside its name. */
export const sizeText = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;

  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

/** The files of a paste or a drop: `ok` are images, `refused` are the names of the rest. */
export const sortFiles = (files: Iterable<File>) => {
  const ok: File[] = [];
  const refused: string[] = [];

  for (const file of files) {
    if (isImage(file)) ok.push(file);
    else refused.push(file.name);
  }

  return { ok, refused };
};
