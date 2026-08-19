<script lang="ts">
  import { onMount } from "svelte";
  import { error, info } from "@fltsci/tauri-plugin-tracing";

  type Invoke = (
    command: string,
    args?: Record<string, unknown>,
  ) => Promise<any>;
  type PostponeOption = { seconds: number; duration: number; unit: string };
  let invoke: Invoke;
  let seconds = 0;
  let title = "Take a Break";
  let canSkip = false;
  let canPostpone = false;
  let showDistanceGuidance = false;
  let options: PostponeOption[] = [];
  let timer: ReturnType<typeof setInterval>;
  let closing = false;
  let lastCountdown = 0;
  let soundEnabled = false;
  let animateGuidance = true;
  let theme: "system" | "light" | "dark" = "system";
  const colorScheme = matchMedia("(prefers-color-scheme: dark)");
  const startSound = new Audio("/sounds/on_pre_break.wav");
  const stopSound = new Audio("/sounds/on_stop_break.wav");

  function playSound(audio: HTMLAudioElement) {
    if (!soundEnabled) return Promise.resolve();
    audio.currentTime = 0;
    return new Promise<void>((resolve) => {
      audio.addEventListener("ended", () => resolve(), { once: true });
      audio.play().catch(() => resolve());
    });
  }

  function beep() {
    if (!soundEnabled) return;
    const context = new AudioContext();
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.frequency.value = 880;
    gain.gain.setValueAtTime(0.12, context.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, context.currentTime + 0.12);
    oscillator.connect(gain).connect(context.destination);
    oscillator.start();
    oscillator.stop(context.currentTime + 0.12);
    oscillator.addEventListener("ended", () => context.close());
  }

  async function finish(command: string, args?: Record<string, unknown>) {
    if (closing) return;
    closing = true;
    await playSound(stopSound);
    await new Promise((resolve) => setTimeout(resolve, 350));
    await invoke(command, args);
  }

  function updateTimer() {
    if (seconds >= 2 && seconds <= 4 && seconds !== lastCountdown) {
      lastCountdown = seconds;
      beep();
    }
    if (seconds <= 1) {
      seconds = 0;
      finish("complete_break").catch((reason) => error("Failed to complete break", reason));
      return;
    }
    seconds -= 1;
  }

  async function loadBreak() {
    if (!invoke) return;
    clearInterval(timer);
    closing = false;
    lastCountdown = 0;
    const [status, schedule, settings] = await Promise.all([
      invoke("get_engine_status"),
      invoke("get_break_schedule"),
      invoke("get_settings"),
    ]);
    info("GazeGuard break settings", settings);
    soundEnabled = settings?.play_sound === true;
    animateGuidance = settings?.animate_guidance !== false;
    theme = settings?.theme ?? "system";
    document.documentElement.dataset.theme = theme === "system"
      ? (colorScheme.matches ? "dark" : "light") : theme;
    options = schedule.postpone_options ?? [];
    const breakInfo =
      status?.current_break ?? (await invoke("get_current_break_info"));
    seconds = status?.seconds_remaining ?? breakInfo.duration_seconds;
    info("GazeGuard break timer initialized", {
      statusSeconds: status?.seconds_remaining,
      breakDuration: breakInfo.duration_seconds,
      seconds,
    });
    title =
      breakInfo.template_name?.replaceAll("_", " ") ??
      (breakInfo.kind === "long" ? "Take a Long Break" : "Take a Short Break");
    showDistanceGuidance =
      breakInfo.template_name === "Focus on a point in the far distance";
    canSkip = Boolean(status?.can_skip) && !status?.skip_limit_reached;
    canPostpone = Boolean(status?.can_postpone);
    await playSound(startSound);
    updateTimer();
    info("GazeGuard break timer after first tick", { seconds });
    timer = setInterval(updateTimer, 1000);
  }

  onMount(async () => {
    const syncSystemTheme = () => {
      if (theme === "system") document.documentElement.dataset.theme = colorScheme.matches ? "dark" : "light";
    };
    colorScheme.addEventListener("change", syncSystemTheme);
    invoke = window.__TAURI__?.core?.invoke;
    if (!invoke) return () => colorScheme.removeEventListener("change", syncSystemTheme);
    await loadBreak();
    return () => { clearInterval(timer); colorScheme.removeEventListener("change", syncSystemTheme); };
  });

  function formatTime(value: number) {
    return `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;
  }
</script>

<svelte:head><title>Take a Break</title></svelte:head>
<main class="break-screen">
  <div class:static-guidance={!animateGuidance} class="break-content">
    <div class="indicator" aria-hidden="true">
      <div class="pulse-ring"></div>
      <div class="pulse-ring inner"></div>
      <div class="indicator-icon">◉</div>
    </div>
    <h1>{title}</h1>
    <div class="timer">{formatTime(Math.max(0, seconds))}</div>
    {#if showDistanceGuidance}<p>Look at something 6 meters away.</p>{/if}
  </div>
  <div class="actions">
    {#if canSkip}<button onclick={() => finish("skip_break")}>Skip Break</button
      >{/if}
    {#if canPostpone}{#each options as option}<button
          onclick={() => finish("postpone_break", { seconds: option.seconds })}
          >Postpone {option.duration} {option.unit}</button
        >{/each}{/if}
  </div>
</main>

<style>
  :global(html),
  :global(body),
  :global(#app) {
    width: 100%;
    min-height: 100%;
    margin: 0;
    background: #f8f9ff;
  }
  :global(html[data-theme="dark"]),
  :global(html[data-theme="dark"] body),
  :global(html[data-theme="dark"] #app) {
    background: #202124;
  }
  .break-screen {
    --break-bg: #f8f9ff;
    --break-text: #121c2a;
    --break-border: #bfc8c8;
    width: 100%;
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
    box-sizing: border-box;
    text-align: center;
    background: var(--break-bg);
    color: var(--break-text);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }
  :global(html[data-theme="dark"]) .break-screen {
    --break-bg: #202124;
    --break-text: #f6f6f6;
    --break-border: #555b60;
  }
  .break-content {
    display: grid;
    gap: 16px;
    justify-items: center;
    animation: break-fade-in 500ms ease-out both;
  }
  .static-guidance, .static-guidance .pulse-ring { animation: none; }
  .indicator {
    position: relative;
    display: grid;
    place-items: center;
    width: 192px;
    height: 192px;
  }
  .pulse-ring {
    position: absolute;
    inset: 0;
    border: 2px solid currentColor;
    border-radius: 50%;
    opacity: 0.2;
    animation: pulse-ring 4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }
  .pulse-ring.inner {
    inset: 16px;
    border-width: 1px;
    animation-delay: 500ms;
  }
  .indicator-icon {
    display: grid;
    place-items: center;
    width: 96px;
    height: 96px;
    border-radius: 50%;
    background: #4a7c7c;
    color: #ecfffe;
    font-size: 48px;
  }
  @keyframes break-fade-in {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }
  @keyframes pulse-ring {
    0%, 100% { transform: scale(0.95); opacity: 0.2; }
    50% { transform: scale(1.05); opacity: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .break-content, .pulse-ring { animation: none; }
  }
  .timer {
    font-size: 80px;
    font-variant-numeric: tabular-nums;
  }
  .actions {
    position: fixed;
    bottom: 40px;
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    justify-content: center;
  }
  button {
    min-width: 140px;
    padding: 12px 18px;
    border: 2px solid var(--break-border);
    border-radius: 12px;
    background: var(--break-bg);
    color: var(--break-text);
    font-size: 16px;
    cursor: pointer;
  }
</style>
