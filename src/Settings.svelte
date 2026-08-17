<script lang="ts">
  import { onMount } from 'svelte';

  type Theme = 'system' | 'light' | 'dark';
  type Invoke = ((command: string, args?: Record<string, unknown>) => Promise<unknown>) | null;
  export let invoke: Invoke = null;

  let theme: Theme = 'system';
  let status = 'Ready';
  let error = '';
  let soundEnabled = true;

  const toggles = [
    ['Enable notifications', true], ['Eye exercises', true], ['Animate guidance', true],
    ['Play sound (start/end)', false], ['Start at login', true],
    ['Pause during fullscreen', true], ['Pause when idle', false], ['Keep in menu bar', true]
  ] as const;

  function applyTheme(value: Theme) {
    theme = value;
    document.documentElement.dataset.theme = value;
  }

  async function testBreak() {
    if (!invoke) return;
    try { await invoke('show_break_window'); status = 'Break started'; error = ''; }
    catch (e) { error = String(e); }
  }

  async function profile(command: 'start_cpu_profile' | 'stop_cpu_profile') {
    if (!invoke) return;
    try {
      const result = await invoke(command);
      status = command === 'start_cpu_profile' ? 'CPU profiling' : `Profile saved: ${result}`;
      error = '';
    } catch (e) { error = String(e); }
  }

  function setSoundEnabled(event: Event) {
    soundEnabled = (event.currentTarget as HTMLInputElement).checked;
    localStorage.setItem('gazeguard-sound-enabled', String(soundEnabled));
  }

  onMount(async () => {
    applyTheme((localStorage.getItem('gazeguard-theme') as Theme) || 'system');
    soundEnabled = localStorage.getItem('gazeguard-sound-enabled') !== 'false';
    if (invoke) {
      try {
        const engine = await invoke('get_engine_status') as { phase?: string };
        status = engine.phase === 'running' ? 'Running' : 'Ready';
      } catch { status = 'Ready'; }
    }
  });
</script>

<svelte:head><title>GazeGuard Settings</title></svelte:head>

<div class="window">
  <header><div class="icon" aria-hidden="true">◉</div><h1>Settings</h1><p>Protect your eyes while you work</p></header>
  <main>
    <section><h2>Break schedule</h2>
      <label class="row">Short break interval <span><input type="number" value="20" aria-label="Short break interval"> min</span></label>
      <label class="row">Short break duration <span><input type="number" value="20" aria-label="Short break duration"> s</span></label>
      <div class="divider"></div>
      <label class="row">Long break interval <span><input type="number" value="60" aria-label="Long break interval"> min</span></label>
      <label class="row">Long break duration <span><input type="number" value="5" aria-label="Long break duration"> min</span></label>
    </section>
    <section><h2>Break experience</h2>
      {#each toggles.slice(1, 4) as [label, checked]}
        <label class="row">{label}<input class="toggle" type="checkbox" {checked} aria-label={label}></label>
      {/each}
      <label class="row column">Exercise style<select><option>Automatic</option><option>Blinking</option><option>Gaze movement</option><option>Relaxation</option></select></label>
    </section>
    <section><h2>Appearance</h2><label class="row">Theme<select bind:value={theme} onchange={() => { localStorage.setItem('gazeguard-theme', theme); applyTheme(theme); }}><option value="system">Match System</option><option value="light">Light</option><option value="dark">Dark</option></select></label></section>
    <section><h2>Behavior</h2>
      {#each toggles.slice(4) as [label, checked]}
        <label class="row">{label}<input class="toggle" type="checkbox" checked={label === 'Play sound (start/end)' ? soundEnabled : checked} onchange={label === 'Play sound (start/end)' ? setSoundEnabled : undefined} aria-label={label}></label>
      {/each}
    </section>
    <section><h2>Developer profiling</h2><div class="actions"><button onclick={() => profile('start_cpu_profile')}>Start CPU Profile</button><button onclick={() => profile('stop_cpu_profile')}>Stop CPU Profile</button></div></section>
    {#if error}<p class="error">{error}</p>{/if}
  </main>
  <footer><div class="status"><span class="dot"></span>Status: {status}</div><div class="actions"><button onclick={testBreak}>Test Break</button><button class="primary" onclick={() => status = 'Settings saved'}>Save Settings</button></div></footer>
</div>
