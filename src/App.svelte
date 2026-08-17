<script lang="ts">
  import Settings from './Settings.svelte';

  type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
  const invoke: Invoke | null = window.__TAURI__?.core?.invoke ?? null;
</script>

<Settings {invoke} />
