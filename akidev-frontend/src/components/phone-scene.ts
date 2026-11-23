// A real 3D iPhone for the landing page: a dark titanium frame, a black bezel and a screen that shows the app. Built from
// code (no model file) and lit by a studio room. The canvas fills the whole stage. The page sets where the phone is
// (anywhere on the stage, any size, any turn) and which screen shows as the visitor scrolls, and the visitor can also drag
// it. Loaded only on a device that can run WebGL, after the page is up (see feature-story.tsx).
import {
  ACESFilmicToneMapping,
  AmbientLight,
  BoxGeometry,
  DirectionalLight,
  ExtrudeGeometry,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  PerspectiveCamera,
  PMREMGenerator,
  Scene,
  Shape,
  ShapeGeometry,
  SRGBColorSpace,
  TextureLoader,
  WebGLRenderer,
  type BufferGeometry,
} from "three";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";

// Sizes are in screen widths: the screen is 1 wide and 1440 / 616 tall, the film's screen.
const SCREEN_H = 1440 / 616;
const BEZEL = 0.036;
const RING = 0.024; // the metal edge around the bezel
const FRAME_W = 1 + 2 * (BEZEL + RING);
const FRAME_H = SCREEN_H + 2 * (BEZEL + RING);
const DEPTH = 0.13;
const FRAME_R = 0.14 + BEZEL + RING;
const FOV = 28;
/** How far a drag may turn the phone away from its pose, in radians. */
const MAX_DRAG = 0.9;

function roundedRect(w: number, h: number, r: number) {
  const x = -w / 2;
  const y = -h / 2;
  const s = new Shape();
  s.moveTo(x + r, y);
  s.lineTo(x + w - r, y);
  s.absarc(x + w - r, y + r, r, -Math.PI / 2, 0, false);
  s.lineTo(x + w, y + h - r);
  s.absarc(x + w - r, y + h - r, r, 0, Math.PI / 2, false);
  s.lineTo(x + r, y + h);
  s.absarc(x + r, y + h - r, r, Math.PI / 2, Math.PI, false);
  s.lineTo(x, y + r);
  s.absarc(x + r, y + r, r, Math.PI, Math.PI * 1.5, false);
  return s;
}

/** A flat rounded shape whose texture coordinates run 0 to 1 across it, so a picture or video fits it exactly. */
function screenShape(w: number, h: number, r: number) {
  const geometry = new ShapeGeometry(roundedRect(w, h, r), 48);
  const position = geometry.attributes.position;
  const uv = geometry.attributes.uv;
  for (let i = 0; i < position.count; i++) uv.setXY(i, position.getX(i) / w + 0.5, position.getY(i) / h + 0.5);
  return geometry;
}

export type Pose = {
  /** Sideways, from -1 (the left edge of the stage) to 1 (the right edge). */
  x: number;
  /** Up, from -1 (the bottom edge) to 1 (the top edge). */
  y: number;
  /** Turn around the upright axis, in radians. */
  yaw: number;
  /** Tilt forward or back, in radians. */
  pitch: number;
  /** Size, 1 is the size that fits the stage. */
  scale: number;
};

export type PhoneScene = {
  /** Where the phone should be. It gets there on a spring, never in a jump. */
  setPose: (pose: Pose) => void;
  /** Which screen shows, as an index into the pictures that were given. */
  setScreen: (index: number) => void;
  /** Draw frames only while the stage is on screen. */
  setActive: (active: boolean) => void;
  /** How tall the phone is at size 1, as a fraction of the stage's height. */
  heightFraction: () => number;
  dispose: () => void;
};

/** `host` gets the canvas. `events` is the element that hears the drag (the whole stage, so the words and buttons stay usable). */
export function createPhoneScene(host: HTMLElement, events: HTMLElement, pictures: string[]): PhoneScene {
  const renderer = new WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  const canvas = renderer.domElement;
  canvas.style.cssText = "display:block;width:100%;height:100%;pointer-events:none";
  host.appendChild(canvas);

  const scene = new Scene();
  const pmrem = new PMREMGenerator(renderer);
  const room = new RoomEnvironment();
  const environment = pmrem.fromScene(room, 0.04).texture;
  scene.environment = environment;
  const key = new DirectionalLight(0xffffff, 1.1);
  key.position.set(2.5, 3, 4);
  // A light from behind, so the edge of the phone shows against a dark or a bright background.
  const rim = new DirectionalLight(0xffffff, 1.6);
  rim.position.set(-3, 2, -3);
  scene.add(key, rim, new AmbientLight(0xffffff, 0.15));

  const camera = new PerspectiveCamera(FOV, 1, 0.1, 50);
  const toDispose: { dispose: () => void }[] = [pmrem, renderer, environment];
  const own = <T extends { dispose: () => void }>(item: T) => (toDispose.push(item), item);

  // The phone.
  const phone = new Group();
  scene.add(phone);

  const titanium = own(new MeshStandardMaterial({ color: 0x232327, metalness: 1, roughness: 0.34 }));
  const bevel = 0.016;
  const bevelDepth = 0.02;
  const frameGeometry: BufferGeometry = own(
    new ExtrudeGeometry(roundedRect(FRAME_W - 2 * bevel, FRAME_H - 2 * bevel, FRAME_R - bevel), {
      depth: DEPTH - 2 * bevelDepth,
      bevelEnabled: true,
      bevelThickness: bevelDepth,
      bevelSize: bevel,
      bevelSegments: 6,
      curveSegments: 48,
    }),
  );
  frameGeometry.translate(0, 0, -(DEPTH - 2 * bevelDepth) / 2);
  phone.add(new Mesh(frameGeometry, titanium));

  const front = DEPTH / 2;
  const bezelMesh = new Mesh(
    own(new ShapeGeometry(roundedRect(1 + 2 * BEZEL, SCREEN_H + 2 * BEZEL, 0.14 + BEZEL), 48)),
    own(new MeshStandardMaterial({ color: 0x050506, roughness: 0.25, metalness: 0 })),
  );
  bezelMesh.position.z = front + 0.0008;
  phone.add(bezelMesh);

  // The screens: each picture, stacked. Only the chosen one is opaque; the others fade out.
  const screenGeometry = own(screenShape(1, SCREEN_H, 0.14));
  const loader = new TextureLoader();
  const screens = pictures.map((_, index) => {
    // Until its picture arrives, a screen is the dark of the phone, not a white plane.
    const material = own(new MeshBasicMaterial({ color: 0x0d0d0e, transparent: true, opacity: index === 0 ? 1 : 0, depthWrite: false, toneMapped: false }));
    const mesh = new Mesh(screenGeometry, material);
    mesh.position.z = front + 0.0016 + index * 0.00008;
    mesh.renderOrder = index;
    phone.add(mesh);
    return { material, opacity: index === 0 ? 1 : 0 };
  });
  // The pictures come one after the other, and each goes to the graphics card as soon as it arrives. The first frame then
  // never waits for six large uploads at once, and the pictures never compete with the page for the network.
  let disposed = false;
  const loadPicture = (index: number) => {
    if (index >= pictures.length || disposed) return;
    loader.load(pictures[index], (texture) => {
      if (disposed) return texture.dispose();
      texture.colorSpace = SRGBColorSpace;
      texture.anisotropy = renderer.capabilities.getMaxAnisotropy();
      own(texture);
      renderer.initTexture(texture);
      const { material } = screens[index];
      material.map = texture;
      material.color.set(0xffffff);
      material.needsUpdate = true;
      dirty = true;
      loadPicture(index + 1);
    });
  };
  loadPicture(0);

  // The glass: nearly clear, so the room shows on it as the phone turns.
  const glass = new Mesh(
    screenGeometry,
    own(new MeshPhysicalMaterial({ color: 0x000000, roughness: 0.04, metalness: 0, transparent: true, opacity: 0.1, clearcoat: 1, clearcoatRoughness: 0.05, depthWrite: false })),
  );
  glass.position.z = front + 0.0032;
  glass.renderOrder = 99;
  phone.add(glass);

  // The buttons: action and volume on the left, power on the right (from the top, as fractions of the height).
  const buttonMaterial = own(new MeshStandardMaterial({ color: 0x2c2c30, metalness: 1, roughness: 0.42 }));
  const addButton = (side: -1 | 1, from: number, length: number) => {
    const height = length * FRAME_H;
    const button = new Mesh(own(new BoxGeometry(0.014, height, 0.05)), buttonMaterial);
    button.position.set(side * (FRAME_W / 2 + 0.002), FRAME_H / 2 - (from + length / 2) * FRAME_H, 0);
    phone.add(button);
  };
  addButton(-1, 0.115, 0.032);
  addButton(-1, 0.18, 0.062);
  addButton(-1, 0.26, 0.062);
  addButton(1, 0.22, 0.1);

  // Size the picture to the stage: the whole phone, with room around it.
  const view = { w: 1, h: 1 }; // what the camera sees at the phone's distance, in screen widths
  const fit = () => {
    const w = host.clientWidth || 1;
    const h = host.clientHeight || 1;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    const tan = Math.tan((FOV * Math.PI) / 360);
    const forHeight = FRAME_H / 0.78 / (2 * tan);
    const forWidth = FRAME_W / 0.62 / (2 * tan * camera.aspect);
    const distance = Math.max(forHeight, forWidth);
    camera.position.set(0, 0, distance);
    camera.updateProjectionMatrix();
    view.h = 2 * distance * tan;
    view.w = view.h * camera.aspect;
    dirty = true;
  };

  // Motion: critically damped springs (no bounce, about 0.4 s), so the phone never snaps and always starts from where
  // it is. Each one chases a target: the pose from the page, plus the finger while it drags.
  type Spring = { pos: number; vel: number };
  const spring = (pos = 0): Spring => ({ pos, vel: 0 });
  const px = spring();
  const py = spring();
  const yaw = spring();
  const pitch = spring();
  const size = spring(1);
  let pose: Pose = { x: 0, y: 0, yaw: 0, pitch: 0, scale: 1 };
  let pointerYaw = 0;
  let pointerPitch = 0;
  let dragging = false;
  let dragDelta = 0;
  let shown = 0;
  let dirty = true;
  let active = true;
  let frame = 0;
  let last = performance.now();

  const step = (axis: Spring, target: number, dt: number) => {
    const w = (2 * Math.PI) / 0.38;
    axis.vel += (w * w * (target - axis.pos) - 2 * w * axis.vel) * dt;
    axis.pos += axis.vel * dt;
    return Math.abs(target - axis.pos) > 0.0005 || Math.abs(axis.vel) > 0.0005;
  };

  const tick = (now: number) => {
    frame = requestAnimationFrame(tick);
    if (!active) return;
    const dt = Math.min(0.032, (now - last) / 1000);
    last = now;
    let moving = false;
    moving = step(px, pose.x, dt) || moving;
    moving = step(py, pose.y, dt) || moving;
    moving = step(yaw, pose.yaw + dragDelta + pointerYaw, dt) || moving;
    moving = step(pitch, pose.pitch + pointerPitch, dt) || moving;
    moving = step(size, pose.scale, dt) || moving;
    phone.rotation.set(pitch.pos, yaw.pos, 0);
    phone.position.set((px.pos * view.w) / 2, (py.pos * view.h) / 2, 0);
    phone.scale.setScalar(size.pos);

    // The chosen screen fades in as the others fade out.
    const fade = 1 - Math.exp(-dt * 9);
    screens.forEach((screen, index) => {
      const target = index === shown ? 1 : 0;
      if (Math.abs(target - screen.opacity) > 0.002) {
        screen.opacity += (target - screen.opacity) * fade;
        moving = true;
      } else screen.opacity = target;
      screen.material.opacity = screen.opacity;
      screen.material.visible = screen.opacity > 0.002;
    });

    // Draw only when something moved.
    if (moving || dirty) {
      renderer.render(scene, camera);
      dirty = false;
    }
  };

  // Pointer: drag anywhere on the stage (but not on a link or a button) to turn the phone; a mouse tilts it a little.
  let startX = 0;
  let startDelta = 0;
  const down = (event: PointerEvent) => {
    if ((event.target as HTMLElement).closest("a, button")) return;
    dragging = true;
    startX = event.clientX;
    startDelta = dragDelta;
  };
  const move = (event: PointerEvent) => {
    if (dragging) {
      dragDelta = Math.max(-MAX_DRAG, Math.min(MAX_DRAG, startDelta + (event.clientX - startX) * 0.009));
    } else if (event.pointerType === "mouse") {
      const box = events.getBoundingClientRect();
      pointerYaw = ((event.clientX - box.left) / box.width - 0.5) * 0.2;
      pointerPitch = ((event.clientY - box.top) / box.height - 0.5) * 0.12;
    }
  };
  const up = () => {
    dragging = false;
    dragDelta = 0; // it turns back to where the page wants it
  };
  const leave = () => {
    pointerYaw = 0;
    pointerPitch = 0;
  };
  events.addEventListener("pointerdown", down);
  events.addEventListener("pointermove", move);
  events.addEventListener("pointerleave", leave);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", up);

  const observer = new ResizeObserver(fit);
  observer.observe(host);
  fit();
  frame = requestAnimationFrame(tick);

  return {
    setPose: (next) => {
      pose = next;
    },
    heightFraction: () => FRAME_H / view.h,
    setScreen: (index) => {
      shown = Math.max(0, Math.min(screens.length - 1, index));
    },
    setActive: (value) => {
      active = value;
      last = performance.now();
    },
    dispose: () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      events.removeEventListener("pointerdown", down);
      events.removeEventListener("pointermove", move);
      events.removeEventListener("pointerleave", leave);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      room.dispose();
      toDispose.forEach((item) => item.dispose());
      canvas.remove();
    },
  };
}
