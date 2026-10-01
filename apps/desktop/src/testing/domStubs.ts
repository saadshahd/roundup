// jsdom has no layout, so it has no scrollIntoView; the Rail calls it on a jump.
Element.prototype.scrollIntoView ??= () => undefined;
