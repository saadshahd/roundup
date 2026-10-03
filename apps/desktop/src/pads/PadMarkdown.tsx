import { createMemo } from "solid-js";
import DOMPurify from "dompurify";
import { marked } from "marked";

export default function PadMarkdown(props: {
  text: string;
  label: string;
  onReader?: (element: HTMLDivElement) => void;
}) {
  const html = createMemo(() =>
    DOMPurify.sanitize(marked.parse(props.text, { async: false }), {
      USE_PROFILES: { html: true },
      FORBID_TAGS: ["img", "style"],
      FORBID_ATTR: ["style"],
    }),
  );

  return <div class="pad-markdown" aria-label={props.label} ref={props.onReader} innerHTML={html()} tabIndex={0} />;
}
