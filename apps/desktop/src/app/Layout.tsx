import type { JSX } from "solid-js";

/** Rail left, centre, Shelf right; nesting and columns are never drawn as boxes. The overlay floats over centre and Shelf. */
export const Layout = (props: {
  header: JSX.Element;
  rail: JSX.Element;
  centre: JSX.Element;
  shelf: JSX.Element;
  overlay: JSX.Element;
}) => (
  <div class="window">
    <header class="header">{props.header}</header>
    <div class="columns">
      <section class="rail" aria-label="rail">
        {props.rail}
      </section>
      <section class="centre" aria-label="centre">
        {props.centre}
      </section>
      <section class="shelf" aria-label="shelf">
        {props.shelf}
      </section>
      {props.overlay}
    </div>
  </div>
);
