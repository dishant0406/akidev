// A real 3D iPhone for the landing page: a dark titanium frame, a black bezel and a screen that plays the app's video.
// Built from code (no model file), lit by a studio room, and turned by dragging, by scrolling and a little by the pointer.
// Loaded only on a device that can run WebGL, after the page is up (see hero-stage.tsx).
import {
  ACESFilmicToneMapping,
  AmbientLight,
  BoxGeometry,
  DirectionalLight,
  ExtrudeGeometry,
  Group,
  LinearFilter,
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
  VideoTexture,
  WebGLRenderer,
  type BufferGeometry,
  type Material,
  type Texture,
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
/** How far the phone may turn when dragged, in radians. */
const MAX_YAW = 0.95;

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

export type PhoneScene = {
  /** Where the page scroll has the phone, from -1 to 1. It turns the phone a little. */
  setTurn: (u: number) => void;
  /** Draw frames only while the stage is on screen. */
  setActive: (active: boolean) => void;
  dispose: () => void;
};

export function createPhoneScene(host: HTMLElement, video: HTMLVideoElement): PhoneScene {
  const renderer = new WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  const canvas = renderer.domElement;
  canvas.style.cssText = "display:block;width:100%;height:100%;touch-action:pan-y;cursor:grab";
  host.appendChild(canvas);

  const scene = new Scene();
  const pmrem = new PMREMGenerator(renderer);
  const room = new RoomEnvironment();
  const environment = pmrem.fromScene(room, 0.04).texture;
  scene.environment = environment;
  const key = new DirectionalLight(0xffffff, 1.1);
  key.position.set(2.5, 3, 4);
  scene.add(key, new AmbientLight(0xffffff, 0.15));

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

  const texture: Texture = own(new VideoTexture(video));
  texture.colorSpace = SRGBColorSpace;
  texture.minFilter = LinearFilter;
  texture.generateMipmaps = false;
  const screen = new Mesh(own(screenShape(1, SCREEN_H, 0.14)), own(new MeshBasicMaterial({ map: texture, toneMapped: false })));
  screen.position.z = front + 0.0016;
  phone.add(screen);

  // The glass: nearly clear, so the room shows on it as the phone turns.
  const glassMaterial: Material = own(
    new MeshPhysicalMaterial({ color: 0x000000, roughness: 0.04, metalness: 0, transparent: true, opacity: 0.1, clearcoat: 1, clearcoatRoughness: 0.05, depthWrite: false }),
  );
  const glass = new Mesh(screen.geometry, glassMaterial);
  glass.position.z = front + 0.0024;
  phone.add(glass);

  // The buttons: action and volume on the left, power on the right (from the top, as fractions of the height).
  const buttonMaterial = own(new MeshStandardMaterial({ color: 0x2c2c30, metalness: 1, roughness: 0.42 }));
  const addButton = (side: -1 | 1, from: number, length: number) => {
    const height = length * FRAME_H;
    const geometry = own(new BoxGeometry(0.014, height, 0.05));
    const button = new Mesh(geometry, buttonMaterial);
    button.position.set(side * (FRAME_W / 2 + 0.002), FRAME_H / 2 - (from + length / 2) * FRAME_H, 0);
    phone.add(button);
  };
  addButton(-1, 0.115, 0.032);
  addButton(-1, 0.18, 0.062);
  addButton(-1, 0.26, 0.062);
  addButton(1, 0.22, 0.1);

  // Size the picture to the stage: the whole phone, with room around it.
  const fit = () => {
    const w = host.clientWidth || 1;
    const h = host.clientHeight || 1;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    const tan = Math.tan((FOV * Math.PI) / 360);
    const forHeight = FRAME_H / 0.8 / (2 * tan);
    const forWidth = FRAME_W / 0.62 / (2 * tan * camera.aspect);
    camera.position.set(0, 0, Math.max(forHeight, forWidth));
    camera.updateProjectionMatrix();
    dirty = true;
  };

  // Turning: a critically damped spring (no bounce, about half a second), so it never snaps and always starts from where
  // the phone is. The target is the scroll's turn, plus the pointer's share, or the finger while it drags.
  const yaw = { pos: 0, vel: 0 };
  const pitch = { pos: 0, vel: 0 };
  let turn = 0;
  let pointerYaw = 0;
  let pointerPitch = 0;
  let dragging = false;
  let dragYaw = 0;
  let dirty = true;
  let active = true;
  let frame = 0;
  let last = performance.now();

  const step = (axis: { pos: number; vel: number }, target: number, dt: number) => {
    const w = (2 * Math.PI) / 0.5;
    axis.vel += (w * w * (target - axis.pos) - 2 * w * axis.vel) * dt;
    axis.pos += axis.vel * dt;
    return Math.abs(target - axis.pos) > 0.0005 || Math.abs(axis.vel) > 0.0005;
  };

  const tick = (now: number) => {
    frame = requestAnimationFrame(tick);
    if (!active) return;
    const dt = Math.min(0.032, (now - last) / 1000);
    last = now;
    const movingYaw = step(yaw, dragging ? dragYaw : turn * 0.5 + pointerYaw, dt);
    const movingPitch = step(pitch, dragging ? 0 : pointerPitch, dt);
    phone.rotation.y = yaw.pos;
    phone.rotation.x = pitch.pos;
    // The picture changes while the video plays, so draw every frame then; otherwise only when something moved.
    if (movingYaw || movingPitch || dirty || !video.paused) {
      renderer.render(scene, camera);
      dirty = false;
    }
  };

  // Pointer: drag to turn; a mouse over the stage tilts it a little.
  let startX = 0;
  let startYaw = 0;
  const down = (event: PointerEvent) => {
    dragging = true;
    startX = event.clientX;
    startYaw = yaw.pos;
    dragYaw = yaw.pos;
    canvas.setPointerCapture(event.pointerId);
    canvas.style.cursor = "grabbing";
  };
  const move = (event: PointerEvent) => {
    if (dragging) {
      dragYaw = Math.max(-MAX_YAW, Math.min(MAX_YAW, startYaw + (event.clientX - startX) * 0.009));
    } else if (event.pointerType === "mouse") {
      const box = canvas.getBoundingClientRect();
      pointerYaw = ((event.clientX - box.left) / box.width - 0.5) * 0.3;
      pointerPitch = ((event.clientY - box.top) / box.height - 0.5) * 0.16;
    }
  };
  const up = () => {
    dragging = false;
    canvas.style.cursor = "grab";
  };
  const leave = () => {
    pointerYaw = 0;
    pointerPitch = 0;
  };
  canvas.addEventListener("pointerdown", down);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", up);
  canvas.addEventListener("pointerleave", leave);

  const observer = new ResizeObserver(fit);
  observer.observe(host);
  fit();
  frame = requestAnimationFrame(tick);

  return {
    setTurn: (u) => {
      turn = Math.max(-1, Math.min(1, u));
    },
    setActive: (value) => {
      active = value;
      last = performance.now();
    },
    dispose: () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      canvas.removeEventListener("pointerdown", down);
      canvas.removeEventListener("pointermove", move);
      canvas.removeEventListener("pointerup", up);
      canvas.removeEventListener("pointercancel", up);
      canvas.removeEventListener("pointerleave", leave);
      room.dispose();
      toDispose.forEach((item) => item.dispose());
      canvas.remove();
    },
  };
}
