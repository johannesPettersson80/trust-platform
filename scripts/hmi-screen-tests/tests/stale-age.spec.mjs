// Spec 39 §11.1 and §19.6: stale age is urgent state, never deferred ordinary value painting.
import {test,expect} from '@playwright/test';
test('a stale value keeps advancing its visible age while values stop arriving',async({page},info)=>{
  await page.addInitScript(()=>{
    const Native=window.WebSocket;
    window.WebSocket=class extends Native {constructor(...args){super(...args);window.__hmiAgeSocket=this;}};
  });
  await page.goto('/hmi/overview');
  const card=page.locator('[data-object="value-1"]');
  await expect(card).toHaveAttribute('data-state','confirmed');
  await page.evaluate(()=>{if(!window.__hmiAgeSocket)throw new Error('No runtime socket');window.__hmiAgeSocket.onmessage=null;});
  await expect(card).toHaveAttribute('data-stale','true',{timeout:10000});
  await expect(card).toContainText(/2 s/,{timeout:5000});
  await expect(card).toContainText(/3 s/,{timeout:3000});
  const bar = page.locator('.hmi-bar').first();
  const label = await bar.locator('[data-part="label"]').boundingBox();
  const age = await bar.locator('text').filter({hasText:/^3 s$/}).boundingBox();
  expect(label).toBeTruthy(); expect(age).toBeTruthy();
  expect(age.y).toBeGreaterThanOrEqual(label.y + label.height);
  await page.screenshot({path:info.outputPath('stale-age.png')});
});
