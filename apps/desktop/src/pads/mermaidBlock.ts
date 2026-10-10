import type { VisualMount } from "./visualBlocks";

/** The calls the Pad makes of Mermaid. */
export type MermaidApi = {
  initialize(config: { startOnLoad: boolean; securityLevel: "strict"; theme: "base"; themeVariables: Record<string, string> }): void;
  parse(text: string): Promise<void>;
  render(id: string, text: string): Promise<{ svg: string }>;
};

let counter = 0;

const token = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/** The error's first line: Mermaid's own message continues with a code frame. */
const firstLine = (error: Error) => error.message.split("\n")[0]?.trim() || "invalid diagram";

const render = async (mermaid: MermaidApi, text: string) => {
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    themeVariables: {
      background: token("--ground"),
      primaryColor: token("--sunken"),
      primaryTextColor: token("--text"),
      primaryBorderColor: token("--grey"),
      lineColor: token("--grey"),
      secondaryColor: token("--sunken"),
      tertiaryColor: token("--ground"),
      textColor: token("--text"),
      fontFamily: token("--font-ui"),
    },
  });
  // `parse` first: a failed `render` leaves its error diagram in the document.
  await mermaid.parse(text);

  return (await mermaid.render(`pad-mermaid-${counter++}`, text)).svg;
};

/** U148: the block as SVG, or its text and one error line. */
export const mountMermaid = (mermaid: MermaidApi): VisualMount => (host, { text }) => {
  let live = true;

  host.classList.add("visual-mermaid");
  void render(mermaid, text).then(
    (svg) => {
      // Mermaid's `strict` level has already sanitized the SVG.
      if (live) host.innerHTML = svg;
    },
    (error: Error) => {
      if (!live) return;
      const source = document.createElement("pre");
      const line = document.createElement("p");

      source.textContent = text;
      line.className = "visual-error";
      line.setAttribute("role", "alert");
      line.textContent = `Mermaid: ${firstLine(error)}`;
      host.replaceChildren(source, line);
    },
  );

  return () => {
    live = false;
  };
};
