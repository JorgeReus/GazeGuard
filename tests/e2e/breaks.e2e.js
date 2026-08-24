describe('breaks', () => {
  beforeEach(async () => {
    await browser.tauri.switchWindow('main');
    const testBreak = await $('button=Test Break');
    await testBreak.waitForExist({ timeout: 10000 });
    await testBreak.click();
    await browser.waitUntil(async () => (await browser.tauri.listWindows()).includes('break'), { timeout: 10000 });
    await browser.tauri.switchWindow('break');
  });

  afterEach(async () => {
    try {
      await browser.tauri.switchWindow('break');
      const skip = await $('button=Skip Break');
      if (await skip.isExisting()) await skip.click();
    } catch {}
    await browser.tauri.switchWindow('main');
  });

  it('opens a fullscreen break and exposes postpone actions', async () => {
    await expect($('h1')).toBeDisplayed();
    await expect($('.timer')).toBeDisplayed();
    await expect($('button=Postpone 5 minutes')).toBeDisplayed();
    await expect(await browser.tauri.listWindows()).toContain('break');
  });

  it('postpones and returns to settings', async () => {
    await $('button=Postpone 5 minutes').click();
    await browser.tauri.switchWindow('main');
    await expect($('h1')).toHaveText('Settings');
  });

  it('skips when skip is allowed', async () => {
    await $('button=Skip Break').click();
    await browser.tauri.switchWindow('main');
    await expect($('h1')).toHaveText('Settings');
  });
});
