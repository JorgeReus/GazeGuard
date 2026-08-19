<script lang="ts">
  import { onMount } from 'svelte';

  type Theme = 'system' | 'light' | 'dark';
  type Invoke = ((command: string, args?: Record<string, unknown>) => Promise<unknown>) | null;
  export let invoke: Invoke = null;

  let theme: Theme = 'system';
  let status = 'Ready';
  let error = '';
  let soundEnabled = false;
  let settings: Record<string, any> = {};

  const toggles = [
    ['Enable notifications', true], ['Eye exercises', true], ['Animate guidance', true],
    ['Play sound (start/end)', false]
  ] as const;

  const behaviorToggles = [
    ['Start at login', 'start_at_login'],
    ['Pause during fullscreen', 'pause_during_fullscreen'],
    ['Pause when idle', 'pause_when_idle'],
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
    updateSetting('play_sound', soundEnabled);
  }

  onMount(async () => {
    applyTheme((localStorage.getItem('gazeguard-theme') as Theme) || 'system');
    if (invoke) {
      try {
        settings = await invoke('get_settings') as Record<string, any>;
        soundEnabled = settings.play_sound !== false;
        const engine = await invoke('get_engine_status') as { phase?: string };
        status = engine.phase === 'running' ? 'Running' : 'Ready';
      } catch { soundEnabled = false; status = 'Ready'; }
    }
  });

  async function updateSetting(key: string, value: unknown) {
    if (!invoke) return;
    settings = { ...settings, [key]: value };
    try { await invoke('update_settings', { settings }); }
    catch (e) { error = String(e); }
  }

  function updateBreakList(key: 'short_breaks' | 'long_breaks', event: Event) {
    const names = (event.currentTarget as HTMLTextAreaElement).value
      .split('\n').map((name) => name.trim()).filter(Boolean);
    updateSetting(key, names.map((name) => ({ name })));
  }

  function updatePostponeOptions(event: Event) {
    const durations = (event.currentTarget as HTMLInputElement).value
      .split(',').map((value) => Number(value.trim())).filter((value) => Number.isFinite(value) && value > 0);
    updateSetting('postpone_options', durations.map((duration) => ({ duration, unit: 'minutes' })));
  }
</script>

<svelte:head><title>GazeGuard Settings</title></svelte:head>

<div class="window">
  <header><div class="icon" aria-hidden="true">◉</div><h1>Settings</h1><p>Protect your eyes while you work</p></header>
  <main>
    <section><h2>Break schedule</h2>
      <label class="row">Short break interval <span><input type="number" value={settings.short_break_interval ?? ''} onchange={(e) => updateSetting('short_break_interval', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Short break interval"> min</span></label>
      <label class="row">Short break duration <span><input type="number" value={settings.short_break_duration ?? ''} onchange={(e) => updateSetting('short_break_duration', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Short break duration"> s</span></label>
      <div class="divider"></div>
      <label class="row">Long break interval <span><input type="number" value={settings.long_break_interval ?? ''} onchange={(e) => updateSetting('long_break_interval', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Long break interval"> min</span></label>
      <label class="row">Long break duration <span><input type="number" value={settings.long_break_duration ?? ''} onchange={(e) => updateSetting('long_break_duration', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Long break duration"> min</span></label>
      <label class="row">Pre-break warning <span><input type="number" value={settings.pre_break_warning_time ?? ''} onchange={(e) => updateSetting('pre_break_warning_time', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Pre-break warning"> s</span></label>
      <label class="row">Consecutive skip limit <span><input type="number" min="0" value={settings.consecutive_skip_limit ?? ''} onchange={(e) => updateSetting('consecutive_skip_limit', Number((e.currentTarget as HTMLInputElement).value))} aria-label="Consecutive skip limit"></span></label>
    </section>
    <section><h2>Postpone options</h2>
      <label class="row column">Durations in minutes<input type="text" value={(settings.postpone_options ?? []).map((option: any) => option.duration).join(', ')} onchange={updatePostponeOptions} aria-label="Postpone durations"></label>
      <p class="hint">Leave empty to disable postponing.</p>
    </section>
    <section><h2>Break content</h2>
      <label class="row column">Short breaks<textarea value={(settings.short_breaks ?? []).map((item: any) => item.name).join('\n')} onchange={(event) => updateBreakList('short_breaks', event)} aria-label="Short break exercises"></textarea></label>
      <label class="row column">Long breaks<textarea value={(settings.long_breaks ?? []).map((item: any) => item.name).join('\n')} onchange={(event) => updateBreakList('long_breaks', event)} aria-label="Long break exercises"></textarea></label>
    </section>
    <section><h2>Break experience</h2>
      {#each toggles.slice(1, 4) as [label, checked]}
        <label class="row">{label}<input class="toggle" type="checkbox" checked={label === 'Play sound (start/end)' ? soundEnabled : checked} onchange={label === 'Play sound (start/end)' ? setSoundEnabled : undefined} aria-label={label}></label>
      {/each}
      <label class="row column">Exercise style<select><option>Automatic</option><option>Blinking</option><option>Gaze movement</option><option>Relaxation</option></select></label>
    </section>
    <section><h2>Appearance</h2><label class="row">Theme<select bind:value={theme} onchange={() => { localStorage.setItem('gazeguard-theme', theme); applyTheme(theme); }}><option value="system">Match System</option><option value="light">Light</option><option value="dark">Dark</option></select></label></section>
    <section><h2>Behavior</h2>
      <label class="row">Random break order<input class="toggle" type="checkbox" checked={settings.random_order === true} onchange={(event) => updateSetting('random_order', (event.currentTarget as HTMLInputElement).checked)} aria-label="Random break order"></label>
      <label class="row">Strict breaks<input class="toggle" type="checkbox" checked={settings.strict_break === true} onchange={(event) => updateSetting('strict_break', (event.currentTarget as HTMLInputElement).checked)} aria-label="Strict breaks"></label>
      <label class="row">Log level<select value={settings.log_level ?? 'info'} onchange={(event) => updateSetting('log_level', (event.currentTarget as HTMLSelectElement).value)} aria-label="Log level"><option value="off">Off</option><option value="error">Error</option><option value="warn">Warn</option><option value="info">Info</option><option value="debug">Debug</option><option value="trace">Trace</option></select></label>
      {#each behaviorToggles as [label, key]}
        <label class="row" title={key === 'pause_during_fullscreen' ? 'Coming soon' : undefined}>
          {label}{key === 'pause_during_fullscreen' ? ' (Coming soon)' : ''}
          <input class="toggle" type="checkbox" checked={settings[key] === true} disabled={key === 'pause_during_fullscreen'} onchange={(event) => updateSetting(key, (event.currentTarget as HTMLInputElement).checked)} aria-label={label}>
        </label>
      {/each}
    </section>
    <section><h2>Developer profiling</h2><div class="actions"><button onclick={() => profile('start_cpu_profile')}>Start CPU Profile</button><button onclick={() => profile('stop_cpu_profile')}>Stop CPU Profile</button></div></section>
    {#if error}<p class="error">{error}</p>{/if}
  </main>
  <footer><div class="status"><span class="dot"></span>Status: {status}</div><div class="actions"><button onclick={testBreak}>Test Break</button><button class="primary" onclick={() => status = 'Settings saved'}>Save Settings</button></div></footer>
</div>
