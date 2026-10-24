import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { captureDir } from "../testing/captures";
import { expect, it } from "vitest";
import { browser, read, url, wait } from "../ui/tabsBrowser";

const capture = async (id: string, step: string) => {
  const root = captureDir(id);
  const paired = id === "U3756a" && /-(before|arrived)$/.test(step);
  const dir = root && (paired ? join(root, process.env.U3756_BASELINE ? "base" : "head") : root);

  if (!dir) return;
  await mkdir(dir, { recursive: true });
  await browser("screenshot", join(dir, `${step}.png`));
  await writeFile(join(dir, `${step}.checks.json`), JSON.stringify(await read(`window.__checks()`), null, 2));
};

const load = async (seed = "empty-project", recording?: string) => {
  await browser("set", "viewport", "1512", "950");
  await browser("set", "media", "dark");
  await browser("open", url(`/harness.html?seed=${seed}&renderer=dom`, process.env.U3756_BASELINE === "1"));

  // agent-browser start creates a new context and drops media emulation; restart records this dark context.
  if (recording) await browser("record", "restart", recording);

  await wait(`!!window.__fake && !!document.querySelector('.rail-actions button')`);
  await read(`(() => {
    const app = window.__fake.app;
    const start = app.handlers['rail.startDoor'];
    app.handlers['rail.startDoor'] = params => new Promise(resolve => { window.arrive = () => resolve(start(params)); });
    document.querySelector('.rail-actions button').click();
  })()`);
  await wait(`!!window.arrive && !!document.querySelector('.rail-row[data-selected=true]')`);
};

it.each(["empty-project", "tree-40"])("u3756a_late_door_terminal_keeps_the_newer_bubble_and_input_%s", async seed => {
  const dir = captureDir("U3756a");

  const recording = dir && !process.env.U3756_BASELINE;

  if (recording) await mkdir(dir, { recursive: true });
  await load(seed, recording ? join(dir, `${seed}.webm`) : undefined);

  try {

  await browser("press", "Meta+t");
  await wait(`!!document.activeElement?.closest('.bubble') && !!window.__terminals.get('101')`);
  await read(`window.selectedWorkstream = document.querySelector('.rail-row[data-selected=true]').dataset.id`);
  await browser("keyboard", "type", "before");
  await wait(`window.__fake.app.calls.some(c => c.method === 'terminal.write')`);
  await read(`window.__fake.writeOutput('101', 'DRAWING BEFORE ARRIVAL' + String.fromCharCode(13, 10))`);
  await wait(`document.querySelector('.bubble[data-bubble="101"] .xterm-rows').textContent.includes('DRAWING BEFORE ARRIVAL')`);
  await capture("U3756a", `${seed}-before`);
  await read(`window.bubbleFocus = document.activeElement; window.arrive()`);
  await wait(`!!document.querySelector('.pane-screen .xterm')`);
  await capture("U3756a", `${seed}-arrived`);
  expect(await read(`document.activeElement === window.bubbleFocus`)).toBe(true);
  expect(await read(`document.querySelector('.rail-row[data-selected=true]').dataset.id === window.selectedWorkstream`)).toBe(true);
  expect(await read(`window.__fake.app.outputSubscriptions.some(s => s.id === '101' && !s.released)`)).toBe(true);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.close')`)).toEqual([]);
  expect(await read(`document.querySelector('.terminal-tab[data-bubble="101"]').getAttribute('aria-pressed')`)).toBe("true");
  await browser("keyboard", "type", "after");
  await wait(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('') === 'beforeafter'`);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').every(c => c.params.id === '101')`)).toBe(true);
  await read(`window.__fake.writeOutput('101', 'DRAWING AFTER ARRIVAL' + String.fromCharCode(13, 10))`);
  await wait(`document.querySelector('.bubble[data-bubble="101"] .xterm-rows').textContent.includes('DRAWING AFTER ARRIVAL')`);
  expect(await read(`document.activeElement === window.bubbleFocus && document.activeElement.checkVisibility()`)).toBe(true);
  await capture("U3756a", `${seed}-after`);

  if (seed === "tree-40") {
    await read(`document.querySelector('.terminal-tab[data-bubble="101"]').click()`);
    await wait(`document.querySelector('.terminal-tab[data-bubble="101"]').getAttribute('aria-pressed') === 'false'`);
    await read(`window.__fake.app.handlers['terminal.open'] = () => { throw new Error('Terminal could not open: retry New Terminal'); }`);
    await browser("press", "Meta+t");
    await wait(`document.body.textContent.includes('Terminal could not open: retry New Terminal')`);
    await capture("U3756a", "tree-40-failure");
  }
  } finally { if (recording) await browser("record", "stop"); }
}, 90_000);

const fresh = async (seed = "tree-40") => {
  await browser("set", "viewport", "1512", "950");
  await browser("set", "media", "dark");
  await browser("open", url(`/harness.html?seed=${seed}&renderer=dom`, process.env.U3756_BASELINE === "1"));
  await wait(`!!window.__fake && !!document.querySelector('.terminal-new')`);
};

const newer = async (choice: string) => {
  if (choice === "bubble") {
    await browser("press", "Meta+t");
    await wait(`!!document.activeElement?.closest('.bubble')`);
  } else if (choice === "ask") {
    await browser("press", "Meta+/");
    await wait(`document.activeElement?.matches('.ask-field') || !!document.activeElement?.closest('.ask-bar')`);
  } else if (choice === "drawer") {
    await read(`(async () => {
      const app = window.__fake.app;
      const made = await app.rpc('todo.create', {title:'Newer Drawer',body:null,blockers:null});
      window.newTodo = made.id;
      const selected = document.querySelector('.rail-row[data-selected=true]')?.closest('[data-id]')?.dataset.id;
      if (selected) await app.rpc('todo.move', {id:made.id,home:selected});
      [...document.querySelectorAll('[role=tab]')].find(t => t.textContent.startsWith('Todos')).click();
    })()`);
    await wait(`!!document.querySelector('[data-todo="' + window.newTodo + '"] .todo-row-button')`);
    await read(`document.querySelector('[data-todo="' + window.newTodo + '"] .todo-row-button').click()`);
    await wait(`!!document.querySelector('.drawer[data-open=true]')`);
    await wait(`!!document.querySelector('.drawer .todo-add-notes')`);
    await read(`document.querySelector('.drawer .todo-add-notes').click()`);
    await wait(`!!document.querySelector('.drawer textarea')`);
    await read(`document.querySelector('.drawer textarea').focus()`);
    await browser('keyboard', 'type', ' exact newer drawer é 😀 ');
  } else {
    await read(`document.querySelector('.rail-row:not([data-selected=true])[tabindex], .rail-row[tabindex], .rail-row [tabindex], .rail [role=treeitem][tabindex]')?.focus()`);
    expect(await read(`!!document.activeElement?.closest('.rail') && document.activeElement.matches('[tabindex]')`)).toBe(true);
  }

  await read(`window.newerFocus = document.activeElement; window.newerText = document.activeElement.value ?? document.activeElement.textContent`);
};

for (const choice of ["drawer", "ask", "rail"]) {
  it(`u3756b_older_appearance_cannot_replace_a_newer_focus_choice_${choice}`, async () => {
    await load(choice === "rail" ? "tree-40" : "empty-project");
    await newer(choice);
    await read(`window.arrive()`);
    await wait(`!!document.querySelector('.pane-screen .xterm')`);
    expect(await read(`document.activeElement === window.newerFocus && (document.activeElement.value ?? document.activeElement.textContent) === window.newerText`)).toBe(true);
    expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write')`)).toEqual([]);
    await capture("U3756b", choice);
  }, 90_000);
}

it.each(["origin", "body"])("u3756c_returning_to_origin_or_body_does_not_rearm_old_focus_%s", async ending => {
  await browser("open", url("/harness.html?seed=tree-40&renderer=dom"));
  await wait(`!!window.__fake && !!document.querySelector('.rail-row[data-id=backend]')`);
  await browser("click", ".rail-row[data-id=backend]");
  await wait(`!!document.querySelector('.pane-action')`);
  await read(`(() => {
    const app = window.__fake.app;
    const start = app.handlers['rail.startDoor'];
    app.handlers['rail.startDoor'] = params => new Promise(resolve => { window.arrive = () => resolve(start(params)); });
    window.originFocus = document.querySelector('.rail-row[data-id=backend]');
    window.originFocus.focus();
    document.querySelector('.pane-action').click();
  })()`);
  await wait(`!!window.arrive`);
  await choose("bubble");
  await read(`${ending === "origin" ? "window.originFocus.focus()" : "document.activeElement.blur()"}; window.expectedFocus = document.activeElement; window.arrive()`);
  await wait(`!!document.querySelector('.pane-screen .xterm')`);
  expect(await read(`document.activeElement === window.expectedFocus`)).toBe(true);
  await capture("U3756c", ending);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').length`)).toBe(0);
}, 90_000);

for (const listFirst of [false, true]) {
  for (const intervene of [false, true]) {
    it(`u3756${intervene ? "b" : "d"}_a_late_user_terminal_open_${listFirst ? "list" : "reply"}_first_${intervene ? "keeps_the_newer_drawer" : "takes_initial_focus"}`, async () => {
      await fresh();
      await read(`(() => {
        const app = window.__fake.app, open = app.handlers['terminal.open'];
        app.handlers['terminal.open'] = params => new Promise(resolve => {
          ${listFirst ? "const result = open(params); window.finishOpen = () => resolve(result);" : "window.finishOpen = () => resolve(open(params));"}
        });
      })()`);
      await browser("press", "Meta+t");
      await wait(`!!window.finishOpen`);

      if (intervene) await newer("drawer");
      await read(`window.finishOpen()`);
      await wait(`!!document.querySelector('.terminal-tab[data-bubble="101"]')`);
      // The wanted effect queues Bubble focus after list reconciliation (Bubbles.tsx).
      await read(`new Promise(resolve => queueMicrotask(resolve))`);

      if (intervene) {
        expect(await read(`document.activeElement === window.newerFocus && (document.activeElement.value ?? document.activeElement.textContent) === window.newerText`)).toBe(true);
        await read(`document.querySelector('.terminal-tab[data-bubble="101"]').click()`);
        await wait(`!!document.activeElement?.closest('.bubble[data-bubble="101"]')`);
      } else await wait(`!!document.activeElement?.closest('.bubble[data-bubble="101"]')`);
      await browser("keyboard", "type", "one");
      await wait(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('') === 'one'`);
      expect(await read(`({ text:window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join(''), same:window.__fake.app.calls.filter(c => c.method === 'terminal.write').every(c => c.params.id === '101') })`)).toEqual({text:"one",same:true});
    }, 90_000);
  }
}

it("u3756d_initial_and_explicit_terminal_focus_still_work", async () => {
  await load();
  await read(`window.arrive()`);
  await wait(`!!document.activeElement?.closest('.pane-screen')`);
  await browser("keyboard", "type", "initial");
  await wait(`window.__fake.app.calls.some(c => c.method === 'terminal.write')`);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('')`)).toBe("initial");
  await capture("U3756d", "initial");
  await newer("bubble");
  await read(`window.originalRow = document.querySelector('.rail-row[data-selected=true]'); document.querySelector('[data-todo-root]').click()`);
  await wait(`!document.querySelector('.rail-row[data-selected=true]')`);
  await read(`window.originalRow.click()`);
  await wait(`!!document.activeElement?.closest('.pane-screen')`);
  await browser("keyboard", "type", "selected");
  await wait(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('') === 'initialselected'`);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('')`)).toBe("initialselected");
  await capture("U3756d", "explicit");
}, 90_000);

for (const intervene of [false, true]) {
  it(`u3756${intervene ? "b" : "d"}_rich_pad_initialization_${intervene ? "yields" : "focuses"}`, async () => {
    await fresh();
    await read(`(() => {
      const app = window.__fake.app, read = app.handlers['pad.read'];
      app.handlers['pad.read'] = params => new Promise(resolve => { window.finishPad = () => resolve(read(params)); });
      [...document.querySelectorAll('[role=tab]')].find(t => t.textContent.startsWith('Pads')).click();
    })()`);
    await wait(`!!document.querySelector('.pad-name')`);
    await read(`document.querySelector('[data-pad="pad-3"] .pad-name').click()`);
    await wait(`!!window.finishPad`);

    if (intervene) await newer("bubble");
    await read(`window.finishPad()`);
    await wait(`!!document.querySelector('.drawer .ProseMirror[contenteditable=true]')`);
    // Milkdown's onReady follows its create promise; the editor's toolbar is initialized before focus.
    await wait(`!!document.querySelector('.drawer .milkdown-toolbar') || !!document.querySelector('.drawer [aria-label="Pad body"]')`);

    if (intervene) expect(await read(`document.activeElement === window.newerFocus`)).toBe(true);
    else expect(await read(`!!document.activeElement?.closest('.ProseMirror')`)).toBe(true);
  }, 90_000);
}

for (const path of ["ask", "switcher", "timeline", "rail-name", "todo-title"]) {
  it(`u3756b_queued_${path}_appearance_yields_to_a_newer_rail_choice`, async () => {
    await fresh();

    if (path === "timeline") {
      await read(`document.querySelector('.rail-row[data-id="payments"]').click()`);
      await wait(`!!document.querySelector('.measurement-action')`);
    }

    if (path === "todo-title") await newer("drawer");

    await read(`window.queuedFocus = []; window.realMicrotask = window.queueMicrotask; window.queueMicrotask = fn => window.queuedFocus.push(fn)`);

    if (path === "ask") await browser("press", "Meta+/");
    else if (path === "switcher") {
      await browser("press", "Meta+k");
      await browser("press", "k");
    } else if (path === "timeline") await read(`[...document.querySelectorAll('.measurement-action button')].find(b => b.textContent === 'Timeline').click()`);
    else if (path === "rail-name") await read(`document.querySelector('.rail-row .name').dispatchEvent(new MouseEvent('dblclick', {bubbles:true}))`);
    else await read(`document.querySelector('.todo-detail-title p[tabindex]').dispatchEvent(new MouseEvent('dblclick', {bubbles:true}))`);
    expect(await read(`window.queuedFocus.length`)).toBeGreaterThan(0);
    await read(`window.newerFocus = document.querySelector('.rail-row[tabindex], .rail-row [tabindex], .rail [role=treeitem][tabindex]'); window.newerFocus.focus(); window.queueMicrotask = window.realMicrotask; window.queuedFocus.splice(0).forEach(fn => fn())`);
    // All app microtasks queued by this action have completed before checking its final focus.
    expect(await read(`document.activeElement === window.newerFocus`)).toBe(true);
  }, 90_000);
}

it.each([true, false])("u3756b_changed_files_read_and_library_readiness_respect_focus_permission_%s", async intervene => {
  await fresh("resources");
  await read(`(() => {
    window.__fake.app.handlers['agent.worktreeState'] = () => ({ahead:1,behind:0,dirty:true});
    window.__fake.app.handlers['agent.changedFiles'] = () => new Promise(resolve => { window.finishFiles = () => resolve({files:[{path:'src/file.ts',status:'modified',line:1}]}); });
    document.querySelector('[data-id="worktree-agent"] .name').click();
  })()`);
  await wait(`!!document.querySelector('.worktree-control')`);
  await read(`document.querySelector('.worktree-control').click()`);
  await wait(`!!window.finishFiles`);

  if (intervene) await newer("rail");

  await read(`window.finishFiles()`);
  await wait(`!!document.querySelector('file-tree-container')?.shadowRoot?.querySelector('[data-item-path]')`);
  expect(await read(intervene ? `document.activeElement === window.newerFocus && (document.activeElement.value ?? document.activeElement.textContent) === window.newerText` : `document.activeElement === document.querySelector('file-tree-container')`)).toBe(true);
}, 90_000);

it("u3756d_explicit_supervisor_start_owns_its_initial_terminal_focus", async () => {
  await fresh("door-stopped");
  await wait(`!!document.querySelector('.pane-action')`);
  await read(`document.querySelector('.pane-action').click()`);
  await wait(`!!document.activeElement?.closest('.pane-screen .xterm')`);
  await browser("keyboard", "type", "start");
  await wait(`window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => atob(c.params.data)).join('') === 'start'`);
  expect(await read(`new Set(window.__fake.app.calls.filter(c => c.method === 'terminal.write').map(c => c.params.id)).size`)).toBe(1);
}, 90_000);


const loadDense = async () => {
  await browser("open", url("/harness.html?seed=tree-40&renderer=dom"));
  await wait(`!!window.__fake && !!document.querySelector('.rail-row') && !!document.querySelector('#shelf-tab-todos')`);
  await read(`(() => {
    const app = window.__fake.app;
    const start = app.handlers['rail.startDoor'];
    app.handlers['rail.startDoor'] = params => new Promise(resolve => { window.arrive = () => resolve(start(params)); });
    const tab = document.querySelector('#shelf-tab-todos'); if (tab.getAttribute('aria-expanded') !== 'true') tab.click();
    document.querySelector('.rail-actions button').click();
  })()`);
  await wait(`!!window.arrive`);
  await read(`window.__fake.app.rpc('todo.move', { id: 1, home: document.querySelector('.rail-row[data-selected=true]').dataset.id })`);
  await wait(`!!document.querySelector('.todo-row-button')`);
};


const choose = async (surface: string) => { await newer(surface); await read(`window.newFocus = window.newerFocus; window.newText = window.newerText`); };

const holdFrames = async () => read(`(() => {
  const request = window.requestAnimationFrame;
  const cancel = window.cancelAnimationFrame;
  const frames = new Map();
  let next = -1;
  window.requestAnimationFrame = callback => { const id = next--; frames.set(id, callback); return id; };
  window.cancelAnimationFrame = id => { if (!frames.delete(id)) cancel(id); };
  window.releaseFrames = () => new Promise(resolve => {
    window.requestAnimationFrame = request;
    window.cancelAnimationFrame = cancel;
    request(time => { for (const callback of frames.values()) callback(time); resolve(frames.size); });
  });
})()`);

it.each(["initial", "return"])("u3756b_a_select_focus_frame_yields_to_the_newer_rail_choice_%s", async phase => {
  await browser("open", url("/harness.html?seed=tree-40&renderer=dom"));
  await wait(`!!window.__fake && !!document.querySelector('.rail-row')`);
  await browser("press", "Meta+Comma");
  await wait(`!!document.querySelector('.settings-view .app-select-trigger')`);

  if (phase === "return") {
    await read(`document.querySelector('.settings-view [role=combobox]').click()`);
    await wait(`!!document.activeElement?.closest('.app-select-list')`);
  }

  await holdFrames();
  await read(`${phase === "initial" ? "document.querySelector('.settings-view .app-select-trigger').click()" : "document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true, cancelable: true }))"}; document.querySelector('.rail-row').focus(); window.newFocus = document.activeElement;`);
  expect(await read<number>(`window.releaseFrames()`)).toBeGreaterThan(0);
  expect(await read(`document.activeElement === window.newFocus`)).toBe(true);
}, 90_000);

it.each(["owner", "home"])("u3756b_a_menu_focus_frame_yields_to_a_newer_rail_choice_%s", async kind => {
  if (kind === "owner") await loadDense();
  else {
    await browser("open", url("/harness.html?seed=agents-10&renderer=dom"));
    await wait(`!!window.__fake && !!document.querySelector('.rail [data-id="agent-1"]')`);
    await read(`(async () => {
      window.__fake.actAs({ kind: 'agent', id: 'agent-1', parent: null });
      window.menuTodo = (await window.__fake.app.rpc('todo.create', { title: 'Move this Todo', body: null, blockers: null })).id;
      window.__fake.actAs({ kind: 'user', id: 'you', parent: null });
      await window.__fake.app.rpc('rail.createWorkstream', {name:'Other Home',parent:null});
    })()`);
    await wait(`!!document.querySelector('.rail [data-id="agent-1"] [data-pads-control]')`);
    await read(`(() => { const control = document.querySelector('.rail [data-id="agent-1"] [data-pads-control]'); if (control.getAttribute('aria-expanded') !== 'true') control.click(); })()`);
    await wait(`!!document.querySelector('.rail [data-todo="' + window.menuTodo + '"]')`);
    await browser("hover", '.rail [data-todo]');
    await wait(`!!document.querySelector('.todo-move') && document.querySelector('.todo-move').getAttribute('aria-disabled') !== 'true'`);
  }

  await browser("focus", ".todo-row-button");
  await holdFrames();
  await read(`${kind === "owner" ? "document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'o', code: 'KeyO', bubbles: true, cancelable: true }))" : "document.querySelector('.todo-move').click()"}; document.querySelector('.rail-row').focus(); window.newFocus = document.activeElement;`);
  expect(await read<number>(`window.releaseFrames()`)).toBeGreaterThan(0);
  expect(await read(`document.activeElement === window.newFocus`)).toBe(true);
}, 90_000);

it("u3756b_a_terminal_name_return_frame_yields_to_the_newer_ask_bar", async () => {
  await load();
  await choose("bubble");
  await browser("dblclick", '.terminal-tab[data-bubble="101"] .terminal-tab-name');
  await wait(`document.activeElement?.matches('.terminal-tab-editor input')`);
  await holdFrames();
  await read(`document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true, cancelable: true }))`);
  await choose("ask");
  expect(await read<number>(`window.releaseFrames()`)).toBeGreaterThan(0);
  expect(await read(`document.activeElement === window.newFocus && window.newFocus.value === window.newText`)).toBe(true);
}, 90_000);

it("u3756c_a_new_drawer_editor_mount_keeps_body_unclaimed_by_the_older_terminal", async () => {
  await load();
  await read(`(async () => {
    const app = window.__fake.app;
    const made = await app.rpc('todo.create', {title:'Mounted editor',body:null,blockers:null});
    window.newTodo = made.id;
    await app.rpc('todo.move', {id:made.id,home:document.querySelector('.rail-row[data-selected=true]').dataset.id});
    [...document.querySelectorAll('[role=tab]')].find(t => t.textContent.startsWith('Todos')).click();
  })()`);
  await wait(`!!document.querySelector('[data-todo="' + window.newTodo + '"] .todo-row-button')`);
  await read(`document.querySelector('[data-todo="' + window.newTodo + '"] .todo-row-button').click()`);
  await wait(`!!document.querySelector('.drawer .todo-add-notes')`);
  await read(`(() => {
    const queue = window.queueMicrotask, queued = [];
    window.queueMicrotask = task => queued.push(task);
    document.querySelector('.drawer .todo-add-notes').focus();
    document.querySelector('.drawer .todo-add-notes').click();
    window.queueMicrotask = queue;
    window.editorMounted = document.querySelector('.drawer textarea');
    window.restoreEditor = () => queued.forEach(task => queue(task));
  })()`);
  expect(await read(`!!window.editorMounted && document.activeElement === document.body`)).toBe(true);
  await read(`window.arrive()`);
  await wait(`!!document.querySelector('.pane-screen .xterm')`);
  expect(await read(`document.activeElement === document.body`)).toBe(true);
  await read(`window.restoreEditor()`);
  await wait(`document.activeElement === window.editorMounted`);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write')`)).toEqual([]);
}, 90_000);

it("u3756b_the_queued_agent_editor_yields_to_the_newer_rail_choice", async () => {
  await loadDense();
  await read(`window.arrive()`);
  await wait(`!!document.querySelector('.pane-screen .xterm')`);
  await read(`(() => {
    const queued = [], queue = window.queueMicrotask;
    window.queueMicrotask = task => queued.push(task);
    try {
      document.querySelector('.rail-row[data-selected=true]').dispatchEvent(new KeyboardEvent('keydown', {key:'N',code:'KeyN',metaKey:true,shiftKey:true,bubbles:true}));
    } finally { window.queueMicrotask = queue; }
    window.queuedCount = queued.length;
    window.newFocus = document.querySelector('.rail-row[data-selected=true]');
    window.newFocus.focus();
    queued.forEach(task => queue(task));
  })()`);
  expect(await read(`window.queuedCount`)).toBeGreaterThan(0);
  expect(await read(`!!document.querySelector('.spawn-prompt-field')`)).toBe(true);
  expect(await read(`document.activeElement === window.newFocus`)).toBe(true);
  expect(await read(`window.__fake.app.calls.filter(c => c.method === 'terminal.write')`)).toEqual([]);
}, 90_000);
