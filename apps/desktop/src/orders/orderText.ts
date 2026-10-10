import type { Order } from "@contracts/agent/Order";

/** O6: the one line the Room shows for an order. */
export const orderLine = (order: Order): string =>
  order.kind === "work" ? `${order.ask} must not: ${order.limits.length}` : `clarifying: ${order.question}`;
