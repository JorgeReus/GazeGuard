const invoke = (command, args) => browser.tauri.execute(
  ({ core }, { command, args }) => core.invoke(command, args),
  { command, args },
);

async function waitForBreakClosed() {
  await browser.waitUntil(async () => !(await browser.getWindowHandles()).includes('break'), { timeout: 10000 });
  await waitForMain();
  await browser.waitUntil(async () => {
    return !(await invoke('e2e_break_window_exists'));
  }, { timeout: 10000 });
}

async function configure(overrides) {
  await browser.tauri.switchWindow('main');
  const settings = await invoke('get_settings');
  await invoke('update_settings', { settings: { ...settings, ...overrides } });
  await invoke('reset_e2e_engine');
  await waitForBreakClosed();
  await waitForMain();
}

async function waitForMain() {
  await browser.waitUntil(async () => {
    if (!(await browser.getWindowHandles()).includes('main')) return false;
    try {
      await browser.tauri.switchWindow('main');
      return (await browser.getTitle()) === 'GazeGuard Settings';
    } catch {
      return false;
    }
  }, { timeout: 10000 });
}

async function startBreak() {
  await waitForMain();
  await waitForBreakClosed();
  await invoke('show_break_window');
  await browser.waitUntil(async () => {
    if (!(await browser.getWindowHandles()).includes('break')) return false;
    try {
      await browser.tauri.switchWindow('break');
      return (await browser.getTitle()) === 'Take a Break';
    } catch {
      return false;
    }
  }, { timeout: 10000 });
}

async function returnToMain(button) {
  await $(button).click();
  await waitForBreakClosed();
  await expect($('h1')).toHaveText('Settings');
}

describe('break behavior', () => {
  afterEach(async () => {
    try {
      if ((await browser.getWindowHandles()).includes('break')) await invoke('complete_break');
    } catch {}
    await waitForBreakClosed();
    await waitForMain();
  });

  it('enforces the consecutive skip limit across breaks', async () => {
    await configure({ consecutive_skip_limit: 2, strict_break: false, short_break_duration: 30 });
    for (let i = 0; i < 2; i++) {
      await startBreak();
      await expect($('button=Skip Break')).toBeDisplayed();
      await returnToMain('button=Skip Break');
    }
    await startBreak();
    await expect($('button=Skip Break')).not.toBeExisting();
  });

  it('turns a one-short cycle into a long break', async () => {
    await configure({
      short_break_interval: 1,
      long_break_interval: 2,
      short_break_duration: 30,
      long_break_duration: 1,
      short_breaks: [{ name: 'Short exercise' }],
      long_breaks: [{ name: 'Long exercise' }],
    });
    await startBreak();
    await expect($('h1')).toHaveText('Short exercise');
    await returnToMain('button=Skip Break');
    await startBreak();
    await expect($('h1')).toHaveText('Long exercise');
  });

  it('removes skipping for strict breaks', async () => {
    await configure({ strict_break: true, short_break_duration: 30 });
    await startBreak();
    await expect($('button=Skip Break')).not.toBeExisting();
  });

  it('removes postpone buttons when no options exist', async () => {
    await configure({ strict_break: false, allow_postpone: true, postpone_options: [], short_break_duration: 30 });
    await startBreak();
    await expect($('button*=Postpone')).not.toBeExisting();
  });

  it('accepts every configured postpone option', async () => {
    const options = [{ duration: 1, unit: 'minutes' }, { duration: 2, unit: 'minutes' }];
    for (const duration of [1, 2]) {
      await configure({ strict_break: false, allow_postpone: true, postpone_options: options, short_break_duration: 30 });
      await startBreak();
      await returnToMain(`button=Postpone ${duration} minutes`);
      const status = await invoke('get_engine_status');
      await expect(status.phase).toBe('running');
      await expect(status.current_break).toBeNull();
    }
  });

  it('shows distance guidance for distance exercise', async () => {
    await configure({
      eye_exercises: true,
      random_order: false,
      short_breaks: [{ name: 'Focus on a point in the far distance' }],
      short_break_duration: 30,
    });
    await startBreak();
    await expect($('p=Look at something 6 meters away.')).toBeDisplayed();
  });

  it('hides exercise copy when exercises are disabled', async () => {
    await configure({
      eye_exercises: false,
      short_breaks: [{ name: 'Hidden exercise' }],
      short_break_duration: 30,
    });
    await startBreak();
    await expect($('h1')).toHaveText('Take a Short Break');
  });

  it('disables guidance animation when configured', async () => {
    await configure({ eye_exercises: true, animate_guidance: false, short_break_duration: 30 });
    await startBreak();
    await expect($('.break-content')).toHaveElementClass('static-guidance');
  });

  it('keeps break exercise order when random order is disabled', async () => {
    await configure({
      random_order: false,
      short_break_interval: 1,
      long_break_interval: 3,
      short_break_duration: 30,
      short_breaks: [{ name: 'First exercise' }, { name: 'Second exercise' }],
    });
    await startBreak();
    await expect($('h1')).toHaveText('First exercise');
    await returnToMain('button=Skip Break');
    await startBreak();
    await expect($('h1')).toHaveText('Second exercise');
  });

  it('finishes a two-second break automatically', async () => {
    await configure({ strict_break: false, short_break_duration: 2 });
    await startBreak();
    await browser.pause(3000);
    await browser.tauri.switchWindow('main');
    await expect($('h1')).toHaveText('Settings');
  });
});
