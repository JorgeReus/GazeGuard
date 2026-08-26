describe('settings', () => {
  const input = (label) => $(`input[aria-label="${label}"]`);
  const select = (label) => label === 'Theme'
    ? $('[data-testid="theme"]')
    : $(`select[aria-label="${label}"]`);
  const setSelect = (selector, value) => browser.execute((selector, value) => {
    const select = document.querySelector(selector);
    select.value = value;
    select.dispatchEvent(new Event('input', { bubbles: true }));
    select.dispatchEvent(new Event('change', { bubbles: true }));
  }, selector, value);
  const setTheme = () => setSelect('[data-testid="theme"]', 'dark');
  const setLogLevel = () => setSelect('select[aria-label="Log level"]', 'trace');

  it('persists schedule, content, toggles, theme, and log level', async () => {
    await browser.tauri.switchWindow('main');
    await expect($('h1')).toHaveText('Settings');
    await (await input('Short break interval')).setValue('1');
    await (await input('Short break duration')).setValue('2');
    await (await input('Long break interval')).setValue('2');
    await (await input('Long break duration')).setValue('3');
    await (await input('Pre-break warning')).setValue('1');
    await (await input('Consecutive skip limit')).setValue('1');
    await (await $('[aria-label="Postpone durations"]')).setValue('1, 2');
    await (await $('[aria-label="Short break exercises"]')).setValue('Blink\nLook far');
    await (await $('[aria-label="Long break exercises"]')).setValue('Walk');
    await setLogLevel();
    for (const label of ['Eye exercises', 'Animate guidance', 'Play sound (start/end)', 'Random break order', 'Strict breaks', 'Pause when idle']) {
      const control = await input(label);
      if (await control.isSelected()) await control.click();
    }
    await setTheme();
    await browser.pause(500);
    await $('button=Save Settings').click();
    await expect($('.status')).toHaveText(expect.stringContaining('Settings saved'));
    await browser.refresh();
    await expect(await input('Short break interval')).toHaveValue('1');
    await expect(await select('Theme')).toHaveValue('dark');
    await expect(await select('Log level')).toHaveValue('trace');
    await expect(await $('[aria-label="Postpone durations"]')).toHaveValue('1, 2');
  });

  it('shows fullscreen pause availability and persists supported settings', async () => {
    await browser.tauri.switchWindow('main');
    const userAgent = await browser.execute(() => navigator.userAgent);
    const control = await input('Pause during fullscreen');

    if (/Macintosh|Windows/i.test(userAgent)) {
      await expect(control).not.toBeDisabled();
      const initial = await control.isSelected();
      await control.click();
      await $('button=Save Settings').click();
      await browser.refresh();
      await expect(await input('Pause during fullscreen')).toBeSelected(!initial);

      await (await input('Pause during fullscreen')).click();
      await $('button=Save Settings').click();
    } else {
      await expect(control).toBeDisabled();
      await expect($('[data-testid="fullscreen-pause-hint"]')).toHaveText('macOS and Windows only');
    }
  });
});
