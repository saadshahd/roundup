import { For, Match, Switch } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";

/** O6: the whole order of one Agent or Door. What it shows follows the Rail, so a changed order reaches it. */
export const OrderDrawer = (props: { id: string }) => {
  const { rail } = useConnectedProject();
  const node = () => rail.nodes.find((each) => each.id === props.id);
  const order = () => node()?.work ?? null;

  return (
    <section class="order-drawer" aria-label={`full order of ${node()?.name ?? props.id}`}>
      <Switch>
        <Match when={order()} keyed>
          {(each) =>
            each.kind === "work" ? (
              <>
                <p class="order-ask">{each.ask}</p>
                <ul class="order-limits">
                  <For each={each.limits}>{(limit) => <li>must not: {limit}</li>}</For>
                </ul>
              </>
            ) : (
              <p class="order-ask">{each.question}</p>
            )
          }
        </Match>
      </Switch>
    </section>
  );
};
