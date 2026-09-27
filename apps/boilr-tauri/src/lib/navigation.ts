import { useEffect, useRef, useState } from "react";

/**
 * Controller and arrow-key navigation, so BoilR works in Steam Deck Game Mode without a mouse.
 * The D-pad, left stick or arrow keys move focus to the nearest control in that direction.
 */

type Direction = "up" | "down" | "left" | "right";

export type NavigationActions = {
  onBack: () => void;
  onPrevView: () => void;
  onNextView: () => void;
  onImport: () => void;
};

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), summary, a[href], [tabindex]:not([tabindex="-1"])';

// Standard gamepad mapping (https://w3c.github.io/gamepad/#remapping).
const BUTTON = { a: 0, b: 1, y: 3, lb: 4, rb: 5, up: 12, down: 13, left: 14, right: 15 } as const;
const PAD_LABELS: Record<string, string> = {
  up: "up", down: "down", left: "left", right: "right", a: "A", b: "B", y: "Y", lb: "L1", rb: "R1",
};
const KEY_DIRECTIONS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};
const STICK_DEADZONE = 0.5;
const REPEAT_DELAY_MS = 350;
const REPEAT_INTERVAL_MS = 110;

/** Visually hidden inputs (pixel checkboxes and toggles) navigate by their drawn label. */
const visualRect = (el: Element): DOMRect => {
  const rect = el.getBoundingClientRect();
  if (rect.width > 2 && rect.height > 2) return rect;
  return (el.closest("label") ?? el).getBoundingClientRect();
};

const isVisible = (el: Element): boolean => {
  const r = visualRect(el);
  return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
};

const center = (r: DOMRect) => ({ x: r.left + r.width / 2, y: r.top + r.height / 2 });

/** The nearest focusable element in a direction, preferring ones aligned with the current one. */
const nearest = (from: Element | null, dir: Direction): HTMLElement | null => {
  const candidates = Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el !== from && isVisible(el)
  );
  if (!from || !(from instanceof HTMLElement) || from === document.body) {
    return candidates[0] ?? null;
  }
  const a = visualRect(from);
  const ac = center(a);
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of candidates) {
    const b = visualRect(el);
    const bc = center(b);
    const dx = bc.x - ac.x;
    const dy = bc.y - ac.y;
    const ahead =
      dir === "up" ? b.bottom <= a.top + 1 : dir === "down" ? b.top >= a.bottom - 1 : dir === "left" ? b.right <= a.left + 1 : b.left >= a.right - 1;
    if (!ahead) continue;
    const along = dir === "up" || dir === "down" ? Math.abs(dy) : Math.abs(dx);
    const across = dir === "up" || dir === "down" ? Math.abs(dx) : Math.abs(dy);
    const score = along + across * 3;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  return best;
};

const isTyping = (el: Element | null): boolean =>
  el instanceof HTMLTextAreaElement ||
  (el instanceof HTMLInputElement && !["checkbox", "radio", "button", "submit"].includes(el.type));

const move = (dir: Direction) => {
  const target = nearest(document.activeElement, dir);
  if (target) {
    target.focus({ preventScroll: true });
    (target.closest("label") ?? target).scrollIntoView({ block: "nearest", inline: "nearest" });
  }
};

const activate = () => {
  const el = document.activeElement;
  if (el instanceof HTMLElement && el !== document.body) el.click();
};

/**
 * Turns controller state into navigation, one poll at a time: each press fires once, and held
 * directions repeat after a short delay. Kept free of timers so it can be driven by any loop.
 */
export const createPadProcessor = (
  getActions: () => NavigationActions,
  onInput: (label: string) => void = () => undefined
) => {
  const pressed = new Map<string, number>(); // control -> time of next repeat (Infinity: no repeat)

  return (pads: readonly Gamepad[], now: number) => {
    const down = new Set<string>();
    for (const pad of pads) {
      const b = (i: number) => Boolean(pad.buttons[i]?.pressed);
      const [x = 0, y = 0] = pad.axes;
      if (b(BUTTON.up) || y < -STICK_DEADZONE) down.add("up");
      if (b(BUTTON.down) || y > STICK_DEADZONE) down.add("down");
      if (b(BUTTON.left) || x < -STICK_DEADZONE) down.add("left");
      if (b(BUTTON.right) || x > STICK_DEADZONE) down.add("right");
      if (b(BUTTON.a)) down.add("a");
      if (b(BUTTON.b)) down.add("b");
      if (b(BUTTON.y)) down.add("y");
      if (b(BUTTON.lb)) down.add("lb");
      if (b(BUTTON.rb)) down.add("rb");
    }

    for (const control of down) {
      const next = pressed.get(control);
      const isDirection = control === "up" || control === "down" || control === "left" || control === "right";
      if (next !== undefined && !(isDirection && now >= next)) continue;
      pressed.set(control, isDirection ? now + (next === undefined ? REPEAT_DELAY_MS : REPEAT_INTERVAL_MS) : Infinity);
      document.body.classList.add("nav-controller");
      onInput(`Controller ${PAD_LABELS[control] ?? control}`);
      const a = getActions();
      if (isDirection) move(control);
      else if (control === "a") activate();
      else if (control === "b") a.onBack();
      else if (control === "y") a.onImport();
      else if (control === "lb") a.onPrevView();
      else if (control === "rb") a.onNextView();
    }
    for (const control of Array.from(pressed.keys())) {
      if (!down.has(control)) pressed.delete(control);
    }
  };
};

/** What navigation can see, shown in Settings to diagnose controllers on a Steam Deck. */
export type NavigationStatus = {
  /** The web view offers the Gamepad API at all. */
  gamepadApi: boolean;
  /** Controllers the web view reports. Most only appear after a button press. */
  pads: string[];
  /** The last navigation input received, like "Key ArrowDown" or "Controller A". */
  lastInput: string | null;
};

const samePads = (a: string[], b: string[]) => a.length === b.length && a.every((v, i) => v === b[i]);

/**
 * Wires arrow keys, Enter, Escape and any connected controller to spatial focus movement and
 * BoilR's actions.
 */
export const useNavigation = (actions: NavigationActions): NavigationStatus => {
  const gamepadApi = typeof navigator !== "undefined" && "getGamepads" in navigator;
  const [pads, setPads] = useState<string[]>([]);
  const [lastInput, setLastInput] = useState<string | null>(null);
  const actionsRef = useRef(actions);
  actionsRef.current = actions;

  // Keyboard, including controller layouts that send keys: arrows move (except left/right while
  // typing), Enter presses (checkboxes don't toggle on Enter by default), Escape goes back.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = document.activeElement;
      const dir: Direction | undefined = KEY_DIRECTIONS[e.key];
      if (dir) {
        if (isTyping(el) && (dir === "left" || dir === "right")) return;
        e.preventDefault();
        move(dir);
      } else if (e.key === "Enter" && el instanceof HTMLInputElement && el.type === "checkbox") {
        e.preventDefault();
        el.click();
      } else if (e.key === "Escape") {
        actionsRef.current.onBack();
      } else {
        return;
      }
      document.body.classList.add("nav-controller");
      setLastInput(`Key ${e.key}`);
    };
    const onPointer = () => document.body.classList.remove("nav-controller");
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, []);

  // Controllers: poll the Gamepad API each frame.
  useEffect(() => {
    if (!gamepadApi) return;
    const process = createPadProcessor(() => actionsRef.current, setLastInput);
    let frame = 0;
    const tick = (now: number) => {
      const connected = navigator.getGamepads().filter((p): p is Gamepad => Boolean(p));
      const ids = connected.map((p) => p.id);
      setPads((was) => (samePads(was, ids) ? was : ids));
      process(connected, now);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [gamepadApi]);

  return { gamepadApi, pads, lastInput };
};
