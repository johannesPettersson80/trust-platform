// Generated from editors/vscode/src/hmiBuilder/display by `npm run build:hmi-drawing`; do not edit.
"use strict";
(() => {
  // src/hmiBuilder/display/alarmSound.ts
  var raise = (row) => `${row.path}@${row.since_ms}`;
  var AlarmSound = class {
    constructor(sounds) {
      this.sounds = sounds;
      /** The raises that have sounded once, and the High raises silenced. */
      this.heard = /* @__PURE__ */ new Set();
      this.silenced = /* @__PURE__ */ new Set();
      this.loud = false;
    }
    /** Whether a High alarm sounds now. */
    get sounding() {
      return this.loud;
    }
    /** The alarms after an update. */
    update(summary) {
      if (!this.sounds) return;
      const active = summary?.active ?? [];
      for (const row of active) {
        if (row.sound === "once" && !this.heard.has(raise(row))) {
          this.heard.add(raise(row));
          this.sounds.play("medium");
        }
      }
      const present = new Set(active.map(raise));
      for (const key of [...this.heard, ...this.silenced]) {
        if (!present.has(key)) {
          this.heard.delete(key);
          this.silenced.delete(key);
        }
      }
      const loud = active.some((row) => row.sound === "continuous" && !this.silenced.has(raise(row)));
      if (loud && !this.loud) this.sounds.play("high");
      else if (!loud && this.loud) this.sounds.stop();
      this.loud = loud;
    }
    /** Silence: the High alarms sounding now stop and stay unacknowledged. */
    silence(summary) {
      for (const row of summary?.active ?? []) {
        if (row.sound === "continuous") this.silenced.add(raise(row));
      }
      if (this.loud) this.sounds?.stop();
      this.loud = false;
    }
  };
  function browserSounds(window2) {
    let context;
    let repeat;
    const audio = () => {
      try {
        context ?? (context = new AudioContext());
        if (context.state === "suspended") void context.resume();
        return context;
      } catch {
        return void 0;
      }
    };
    window2.addEventListener("pointerdown", () => void audio(), { capture: true });
    const tone = (frequency, startIn, seconds) => {
      const sound = audio();
      if (!sound) return;
      const oscillator = sound.createOscillator();
      const gain = sound.createGain();
      const start2 = sound.currentTime + startIn;
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.2, start2);
      gain.gain.exponentialRampToValueAtTime(1e-4, start2 + seconds);
      oscillator.connect(gain).connect(sound.destination);
      oscillator.start(start2);
      oscillator.stop(start2 + seconds);
    };
    return {
      play(pattern) {
        if (pattern === "medium") {
          tone(660, 0, 0.25);
          tone(660, 0.35, 0.25);
          return;
        }
        if (repeat !== void 0) return;
        const pulse = () => {
          tone(880, 0, 0.2);
          tone(880, 0.3, 0.2);
        };
        pulse();
        repeat = window2.setInterval(pulse, 1500);
      },
      stop() {
        if (repeat !== void 0) window2.clearInterval(repeat);
        repeat = void 0;
      }
    };
  }

  // src/hmiBuilder/drawing/regionCoordinates.ts
  function screenCoordinates(page) {
    if (!page.regions?.length) return page;
    const offsets = new Map(page.regions.flatMap((region) => region.objects.map((id) => [id, region.at])));
    return {
      ...page,
      regions: void 0,
      object: page.object.map((object) => {
        const offset = offsets.get(object.id);
        return offset ? { ...object, at: [object.at[0] + offset[0], object.at[1] + offset[1]] } : object;
      })
    };
  }

  // src/hmiBuilder/display/surfacePatch.ts
  var SVG_NAMESPACE = "http://www.w3.org/2000/svg";
  function steady(style) {
    return style?.replace(/animation-delay: -[0-9.]+ms;?/g, "");
  }
  function patchSurface(surface, page, render, last, values) {
    const element2 = surface.firstElementChild;
    const sameFrame = last !== void 0 && element2 !== null && last.children.length === page.children.length && element2.children.length === page.children.length && sameAttributes(last.attrs, page.attrs);
    if (!sameFrame) {
      values.clear();
      surface.innerHTML = render(page);
      return page;
    }
    page.children.forEach((child, index) => {
      const before = last.children[index], node = element2.children[index];
      if (/\bhmi-value\b/.test(child.attrs.class ?? "") && child.attrs["data-state"] === "confirmed" && child.attrs["data-stale"] !== "true" && sameAttributes(before.attrs, child.attrs)) {
        if (!sameDrawing(before, child)) values.enqueue(node, before, child);
      } else values.immediate(node, before, child);
    });
    return page;
  }
  function sameDrawing(before, after) {
    return before.tag === after.tag && before.text === after.text && before.children.length === after.children.length && sameAttributes(before.attrs, after.attrs) && before.children.every((child, index) => sameDrawing(child, after.children[index]));
  }
  function sameAttributes(before, after) {
    const names = Object.keys(after);
    return names.length === Object.keys(before).length && names.every((name) => before[name] === after[name]);
  }
  function updateNode(drawn, before, after, render) {
    if (before.tag !== after.tag || before.children.length !== after.children.length || before.text === void 0 !== (after.text === void 0)) {
      redrawNode(drawn, after, render);
      return;
    }
    for (const [name, value] of Object.entries(after.attrs)) {
      const was = before.attrs[name];
      if (was === value) continue;
      if (name === "style" && steady(was) === steady(value)) continue;
      drawn.setAttribute(name, value);
    }
    for (const name of Object.keys(before.attrs)) {
      if (!(name in after.attrs)) drawn.removeAttribute(name);
    }
    if (after.text !== void 0) {
      if (before.text !== after.text) {
        const text2 = drawn.firstChild;
        if (text2?.nodeType === 3 && drawn.childNodes.length === 1) text2.nodeValue = after.text;
        else drawn.textContent = after.text;
      }
      return;
    }
    if (drawn.children.length !== after.children.length) {
      redrawNode(drawn, after, render);
      return;
    }
    after.children.forEach((child, index) => updateNode(drawn.children[index], before.children[index], child, render));
  }
  function redrawNode(drawn, node, render) {
    const holder = drawn.ownerDocument.createElementNS(SVG_NAMESPACE, "svg");
    holder.innerHTML = render(node);
    const fresh = holder.firstElementChild;
    if (fresh) drawn.replaceWith(fresh);
  }

  // src/hmiBuilder/display/valuePaintQueue.ts
  var ValuePaintQueue = class {
    constructor(schedule, paint, perFrame = 96) {
      this.schedule = schedule;
      this.paint = paint;
      this.perFrame = perFrame;
      this.pending = /* @__PURE__ */ new Map();
      this.scheduled = false;
      this.disposed = false;
    }
    enqueue(element2, before, after) {
      if (this.disposed) return;
      this.pending.set(element2, { before: this.pending.get(element2)?.before ?? before, after });
      this.request();
    }
    immediate(element2, before, after) {
      const actual = this.pending.get(element2)?.before ?? before;
      this.pending.delete(element2);
      if (!this.disposed) this.paint(element2, actual, after);
    }
    /** Paint in the current render turn, sharing its layout pass. Larger pages defer overflow. */
    flush() {
      if (this.disposed) return;
      const batch = [...this.pending].slice(0, this.perFrame);
      for (const [element2, update] of batch) {
        this.pending.delete(element2);
        if (element2.isConnected) this.paint(element2, update.before, update.after);
      }
      this.request();
    }
    clear() {
      this.pending.clear();
    }
    dispose() {
      this.disposed = true;
      this.clear();
    }
    request() {
      if (this.scheduled || this.disposed || !this.pending.size) return;
      this.scheduled = true;
      this.schedule(() => {
        this.scheduled = false;
        this.flush();
      });
    }
  };

  // src/hmiBuilder/drawing/cuts.ts
  var collecting;
  function heard(text2) {
    collecting?.said.add(text2);
    return text2;
  }

  // src/hmiBuilder/drawing/fontMetrics.ts
  var FONT_UNITS = 2048;
  var FONT_CHARS = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\xA0\xA1\xA2\xA3\xA4\xA5\xA6\xA7\xA8\xA9\xAA\xAB\xAC\xAE\xAF\xB0\xB1\xB2\xB3\xB4\xB5\xB6\xB7\xB8\xB9\xBA\xBB\xBC\xBD\xBE\xBF\xC0\xC1\xC2\xC3\xC4\xC5\xC6\xC7\xC8\xC9\xCA\xCB\xCC\xCD\xCE\xCF\xD0\xD1\xD2\xD3\xD4\xD5\xD6\xD7\xD8\xD9\xDA\xDB\xDC\xDD\xDE\xDF\xE0\xE1\xE2\xE3\xE4\xE5\xE6\xE7\xE8\xE9\xEA\xEB\xEC\xED\xEE\xEF\xF0\xF1\xF2\xF3\xF4\xF5\xF6\xF7\xF8\xF9\xFA\xFB\xFC\xFD\xFE\xFF\u0100\u0101\u0102\u0103\u0104\u0105\u0106\u0107\u0108\u0109\u010A\u010B\u010C\u010D\u010E\u010F\u0110\u0111\u0112\u0113\u0114\u0115\u0116\u0117\u0118\u0119\u011A\u011B\u011C\u011D\u011E\u011F\u0120\u0121\u0122\u0123\u0124\u0125\u0126\u0127\u0128\u0129\u012A\u012B\u012C\u012D\u012E\u012F\u0130\u0131\u0132\u0133\u0134\u0135\u0136\u0137\u0138\u0139\u013A\u013B\u013C\u013D\u013E\u013F\u0140\u0141\u0142\u0143\u0144\u0145\u0146\u0147\u0148\u014A\u014B\u014C\u014D\u014E\u014F\u0150\u0151\u0152\u0153\u0154\u0155\u0156\u0157\u0158\u0159\u015A\u015B\u015C\u015D\u015E\u015F\u0160\u0161\u0162\u0163\u0164\u0165\u0166\u0167\u0168\u0169\u016A\u016B\u016C\u016D\u016E\u016F\u0170\u0171\u0172\u0173\u0174\u0175\u0176\u0177\u0178\u0179\u017A\u017B\u017C\u017D\u017E\u017F\u0374\u0375\u0376\u037A\u037B\u037C\u037D\u037E\u037F\u0384\u0385\u0386\u0387\u0388\u0389\u038A\u038C\u038E\u038F\u0390\u0391\u0392\u0393\u0394\u0395\u0396\u0397\u0398\u0399\u039A\u039B\u039C\u039D\u039E\u039F\u03A0\u03A1\u03A3\u03A4\u03A5\u03A6\u03A7\u03A8\u03A9\u03AA\u03AB\u03AC\u03AD\u03AE\u03AF\u03B0\u03B1\u03B2\u03B3\u03B4\u03B5\u03B6\u03B7\u03B8\u03B9\u03BA\u03BB\u03BC\u03BD\u03BE\u03BF\u03C0\u03C1\u03C2\u03C3\u03C4\u03C5\u03C6\u03C7\u03C8\u03C9\u03CA\u03CB\u03CC\u03CD\u03CE\u03CF\u03D0\u03D1\u03D2\u03D3\u03D4\u03D5\u03D6\u03D7\u03DC\u03DD\u03F0\u03F1\u03F2\u03F3\u03F4\u03F5\u03F6\u03F9\u03FA\u03FC\u03FD\u03FE\u03FF\u0400\u0401\u0402\u0403\u0404\u0405\u0406\u0407\u0408\u0409\u040A\u040B\u040C\u040D\u040E\u040F\u0410\u0411\u0412\u0413\u0414\u0415\u0416\u0417\u0418\u0419\u041A\u041B\u041C\u041D\u041E\u041F\u0420\u0421\u0422\u0423\u0424\u0425\u0426\u0427\u0428\u0429\u042A\u042B\u042C\u042D\u042E\u042F\u0430\u0431\u0432\u0433\u0434\u0435\u0436\u0437\u0438\u0439\u043A\u043B\u043C\u043D\u043E\u043F\u0440\u0441\u0442\u0443\u0444\u0445\u0446\u0447\u0448\u0449\u044A\u044B\u044C\u044D\u044E\u044F\u0450\u0451\u0452\u0453\u0454\u0455\u0456\u0457\u0458\u0459\u045A\u045B\u045C\u045D\u045E\u045F\u2010\u2011\u2012\u2013\u2014\u2015\u2016\u2017\u2018\u2019\u201A\u201B\u201C\u201D\u201E\u201F\u2020\u2021\u2022\u2023\u2024\u2025\u2026\u2027\u2030\u2031\u2032\u2033\u2034\u2035\u2036\u2037\u2038\u2039\u203A\u20AC\u2190\u2191\u2192\u2193\u2212\u2264\u2265";
  var FONT_WIDTHS = {
    400: "0g00gd0qi10110i1jv10n0h20kr0kr0si11n0ge0q60ge0ki0zw0n50yp0z510r0xr0za0w70z70za0ge0h611n11n11n0t31iy13911815k1520y70xl16g16a0fa0wh1280w61fe16v17i10c17i10m10i10q16c1391k212t12m0zs0kr0ki0kr0qt0py0id0vy0yu0wi0yu0x60l20yw0xn0ds0ds0v80ds1du0xm0y40yu0yu0lf0u10im0xn0vz1ak0v20vz0vf0o90ix0o911n0g00gd0wi0yr1580va0fd0wc0xm1g00pt0x511n11w0r60px11n0p50pd0id0xc0ya0ge0f90hb0rg0x519n1c71e60t31391391391391391391kj15k0y70y70y70y70fa0fa0fa0fa15t16v17i17i17i17i17i11n17i16c16c16c16c12m1060z20vy0vy0vy0vy0vy0vy1g70wi0x60x60x60x60ds0ds0ds0ds0x50xm0y40y40y40y40y411n0y40xn0xn0xn0xn0vz0yu0vz1390vy1390vy1390vy15k0wi15k0wi15k0wi15k0wi15214e15t0yu0y70x60y70x60y70x60y70x60y70x616g0yw16g0yw16g0yw16g0yw16a0xn1860xn0fa0ds0fa0ds0fa0ds0fa0ds0fa0ds1br0rk0wh0ds1280v80tw0w60ds0w60ds0w60jc0w00kg0xu0ds16v0xm16v0xm16v0xm16v0xm17i0y417i0y417i0y41l61kv10m0lf10m0lf10m0lf10i0u110i0u110i0u110i0u110q0im10q0le10q0im16c0xn16c0xn16c0xn16c0xn16c0xn16c0xn1k21ak12m0vz12m0zs0vf0zs0vf0zs0vf0hw0f60fb16v0f90wi0wi0wi0h60wh0fd12a1390ge1601e30n31d11dz1dj0fo1391180wx12r0y70zs16a17i0fa1281391fe16v11c17i16a10c13710q12m1e312t1e217t0fa12m11s0uk0xn0fo0xn11s0xw0vz0x80uk0sz0xn0zc0fo0tw10i0xc0xe0vs0y40yf0yu0we0zi0qi0xn1500wd18919s0fo0xn0y40xn19s1281021041271c212712z1ao0tw0xr0ud1581020wi0ds17i0pm0pm15k1fe0yu15k15k15k0y70y71bv0wx15k10i0fa0fa0wh1rc1s01eb12816v0zr15313910r1180wx1am0y71im0z516v16v12816k1fe16a17i16a10c15k10q0zr1b412t15013u1ih1jb1ag1e810y15k1lq10m0vy0xs0w50ot0yw0x61a30s40xn0xn0uu0wq17t0xh0y40xg0yu0wi0qi0vz1330v20xz0wv1as1bm10m15w0vs0wi1by0vj0x60x60xn0ot0wi0u10ds0ds0ds1gl1f00xn0uu0xn0vz0xg0ke0ke10w0sg1kw1kw0r90pr0eu0eu0dq0eu0p20p20ny0p20ry0ry0w00w00ge0ws1d60ge23z2o30co0p611o0co0p611o0pw0lv0lv11x1ia1bg1ia1bg11n11n11n",
    500: "0f60hb0s410c10r1ki1160ht0kz0kz0tm11z0h90qb0h90l110q0nm0z20zo11c0yb0zu0wi0zt0zu0h90hy11z11z11z0u01jw14c11d15q1520yb0xj16j16d0fi0wq1340w61fx17117m10i17q10v10r11516414c1l213v13m10g0kz0l10kz0r40qc0j60wb0z60wu0z60xf0ll0z90y80ec0ec0vt0ec1ej0y80yd0z60z60m00un0jd0y80wp1b60vp0wq0vt0p20jo0p211z0f60hb0wu0za15x0vo0go0wc0y81g00q00ym11z11t0qg0q011z0ph0pv0j60y50y70h90h30hv0rl0ym1ah1cu1ew0u014c14c14c14c14c14c1l315q0yb0yb0yb0yb0fi0fi0fi0fi16a17117m17m17m17m17m11z17m16416416416413m10t0zu0wb0wb0wb0wb0wb0wb1g20wu0xf0xf0xf0xf0ec0ec0ec0ec0xg0y80yd0yd0yd0yd0yd11z0yd0y80y80y80y80wq0z60wq14c0wb14c0wb14c0wb15q0wu15q0wu15q0wu15q0wu15215816a0z60yb0xf0yb0xf0yb0xf0yb0xf0yb0xf16j0z916j0z916j0z916j0z916d0y818r0y80fi0ec0fi0ec0fi0ec0fi0ec0fi0ec1c80so0wq0ec1340vt0v20w60ec0w60ec0w60ke0wa0m50y10ec1710y81710y81710y81710y817m0yd17m0yd17m0yd1lf1kn10v0m010v0m010v0m010r0un10r0un10r0un10r0un1150jd1150mn1150jd1640y81640y81640y81640y81640y81640y81l21b613m0wq13m10g0vt10g0vt10g0vt0ih0fn0fr1710fr0wu0wu0wu0hy0wq0gp13714c0h91731f40oa1e51fj1ek0gb14c11d0x013x0yb10g16d17m0fi13414c1fx17111x17m16d10i13a11513m1ec13v1ec1810fi13m12c0uy0y80gb0y812c0y50wp0xm0uy0tg0y80zg0gb0v211a0y50xx0vs0yd0z20z60wq1000rc0y816h0x01991ac0gb0y80yd0y81ac13410q10r12x1dj12x1471cp0v20xu0uf15x10q0wu0ec17m0q50q515q1fx0z615q15q15q0yb0yb1c70x015q10r0fi0fi0wq1rf1s11ei13417111015614c10s11d0x01bj0yb1ko0zo17117113416l1fx16d17m16d10i15q1151101b913v15w13z1j21k51ap1ew11715q1m910v0wb0y30wd0p40zs0xf1b40sl0y80y80vm0xa18m0y30yd0xx0z60wu0rc0wq14s0vp0z30xb1bi1cl11a1780wd0wu1cf0vz0xf0xf0y80p40wu0un0ec0ec0ec1gy1fj0y80vm0y80wq0xx0m70m710v0sg1kw1kw0sw0q70fs0fs0em0fs0qy0qs0pn0qs0tp0tp0uc0uc0h90yj1fs0h92502pi0dn0r614o0dn0r614o0ro0mr0mr1291ia1ct1ia1ct11z11z11z",
    550: "0er0ht0sy10h10v1kt11f0i60l40l40u51250hp0qe0hp0lb1140nu0z90zx11m0yk1040wn1041040hp0ic1251251250ug1kd14v11g15t1530yd0xh16l16e0fm0wv13k0w61g717417o10m17u10z10v11d16014v1lj14f14310s0l40lb0l40r90qj0jk0wh0zc0x00zc0xj0lv0zf0yj0em0em0w40em1ev0yi0yi0zc0zc0mb0uy0jq0yj0x21bg0w10x40w00ph0k10ph1250er0ht0x00zj16a0vv0hb0wc0yi1g00q30zc12511s0q30q11250pn0q40jk0yj0y60hp0i00i50ro0zc1aw1d61f90ug14v14v14v14v14v14v1ld15t0yd0yd0yd0yd0fm0fm0fm0fm16j17417o17o17o17o17o12517o1601601601601431141080wh0wh0wh0wh0wh0wh1g00x00xj0xj0xj0xj0em0em0em0em0xm0yi0yi0yi0yi0yi0yi1250yi0yj0yj0yj0yj0x40zc0x414v0wh14v0wh14v0wh15t0x015t0x015t0x015t0x015315n16j0zc0yd0xj0yd0xj0yd0xj0yd0xj0yd0xj16l0zf16l0zf16l0zf16l0zf16e0yj1910yj0fm0em0fm0em0fm0em0fm0em0fm0em1ch0t70wv0em13k0w40vn0w60em0w60em0w60kx0wg0n00y50em1740yi1740yi1740yi1740yi17o0yi17o0yi17o0yi1lj1kj10z0mb10z0mb10z0mb10v0uy10v0uy10v0uy10v0uy11d0jq11d0n911d0jq1600yj1600yj1600yj1600yj1600yj1600yj1lj1bg1430x414310s0w010s0w010s0w00ir0fw0g01740g00x00x00x00ic0wv0he13o14v0hp17m1fm0ov1ep1gb1f20gm14v11g0x114i0yd10s16e17o0fm13k14v1g717412817o16e10m13c11d1431eg14f1eh1850fm14312n0v50yj0gm0yj12n0ya0x20xu0v50tp0yj0zi0gm0vn11o0yj0y60vs0yi0ze0zc0wx10a0rr0yj1770xb19r1am0gm0yj0yi0yj1am13k11311213a1e913a14t1dq0vn0xv0uh16a1130x00em17o0qf0qf15t1g70zc15t15t15t0yd0yd1cc0x115t10v0fm0fm0wv1rg1s21em13k17411m15814v10s11g0x11c00yd1lp0zx17417413k16m1g716e17o16e10m15t11d11m1bc14f16c1411jd1kl1au1f911b15t1mj10z0wh0y80wh0pa1090xj1bm0su0yj0yj0w00xj1900yf0yi0y60zc0x00rr0x415n0w10zn0xj1bu1d211m17w0wo0x01cn0w70xj0xj0yj0pa0x00uy0em0em0em1h51ft0yj0w00yj0x40y60n40n410u0sg1kw1kw0tp0qf0g90g90f20g90rw0ro0qh0ro0uk0uk0th0th0hp0ze1h30hp25i2q70e50s51660e50s51660sk0n70n712g1ia1dh1ia1dh125125125",
    600: "0ec0ia0tr10m1101l511p0ij0l80l80up12a0i50qh0i50lk11j0o20zg10711w0yu10e0ws10f10e0i50iq12a12a12a0ux1ku15e11i15x1530yg0xg16m16f0fr0wz1400w61gh17717q10p17z11411011k15v15e1m114y14l1140l80lk0l80re0qp0jz0wo0zi0x50zi0xn0m40zl0yu0ew0ew0we0ew1f80yt0yn0zi0zi0ml0v90k30yu0xe1br0wd0xh0w70pv0kf0pv12a0ec0ia0x50zt16m0w20hz0wc0yt1g00q710312a11r0pp0q212a0pt0qd0jz0yy0y50i50iw0if0rq1031ba1dh1fm0ux15e15e15e15e15e15e1ln15x0yg0yg0yg0yg0fr0fr0fr0fr16r17717q17q17q17q17q12a17q15v15v15v15v14l11f10m0wo0wo0wo0wo0wo0wo1fx0x50xn0xn0xn0xn0ew0ew0ew0ew0xs0yt0yn0yn0yn0yn0yn12a0yn0yu0yu0yu0yu0xh0zi0xh15e0wo15e0wo15e0wo15x0x515x0x515x0x515x0x515316216r0zi0yg0xn0yg0xn0yg0xn0yg0xn0yg0xn16m0zl16m0zl16m0zl16m0zl16f0yu19c0yu0fr0ew0fr0ew0fr0ew0fr0ew0fr0ew1cq0tr0wz0ew1400we0w80w60ew0w60ew0w60lg0wl0nu0y80ew1770yt1770yt1770yt1770yt17q0yn17q0yn17q0yn1lo1kf1140ml1140ml1140ml1100v91100v91100v91100v911k0k311k0nv11k0k315v0yu15v0yu15v0yu15v0yu15v0yu15v0yu1m11br14l0xh14l1140w71140w71140w70j10g50g81770g90x50x50x50iq0wz0i214415e0i51851g50pg1f91h31fl0gy15e11i0x21530yg11416f17q0fr14015e1gh17712i17q16f10p13d11k14l1el14y1el1890fr14l12x0vc0yu0gy0yt12x0yf0xe0y10vc0tx0yu0zk0gy0w81220yy0yg0vs0yn0zp0zi0x310j0s60yt17y0xm1a91aw0gy0yt0yn0yt1aw14011f11d13m1ez13m15f1eq0w80xw0ui16m11f0x50ew17q0qp0qp15x1gh0zi15x15x15x0yg0yg1ci0x215x1100fr0fr0wz1ri1s21ep14017712915a15e10s11i0x21ch0yg1mq10717717714016m1gh16f17q16f10p15x11k1291be14y16r1441jo1l01az1fl11g15x1ms1140wo0ye0wl0pg10p0xn1c50t20yu0yu0we0xt19e0yq0yn0yf0zi0x50s60xh16i0wd1070xr1c71dj11y18k0wy0x51cv0wf0xn0xn0yu0pg0x50v90ew0ew0ew1hc1g20yu0we0yu0xh0yf0o00o010t0sg1kw1kw0uj0qn0gq0gq0fi0gq0su0sj0rb0sj0vf0vf0sn0sn0i51091ie0i52612qx0en0t517o0en0t517o0tf0nn0nn12m1ia1e51ia1e512a12a12a",
    650: "0dw0ir0uk10s1151lg11y0ix0lc0lc0v912g0ik0qk0ik0lu11y0ob0zn10g1270z410o0wy10q10o0ik0j412g12g12g0vd1lc15y11l1601530yi0xf16o16g0fv0x414h0w61gr17a17s10s18311911511s15r15y1mj15h15311g0lc0lu0lc0rk0qw0kd0wu0zp0xb0zp0xs0me0zs0z50f60f60wp0f61fk0z40yr0zp0zp0mw0vk0kh0z50xr1c20wp0xv0we0qa0ks0qa12g0dw0ir0xb10216z0w90im0wc0z41g00qa10t12g11q0pc0q412g0q00qm0kd0zc0y40ik0jt0ip0rt10t1bp1dt1fz0vd15y15y15y15y15y15y1lx1600yi0yi0yi0yi0fv0fv0fv0fv17017a17s17s17s17s17s12g17s15r15r15r15r15311r1100wu0wu0wu0wu0wu0wu1fv0xb0xs0xs0xs0xs0f60f60f60f60xx0z40yr0yr0yr0yr0yr12g0yr0z50z50z50z50xv0zp0xv15y0wu15y0wu15y0wu1600xb1600xb1600xb1600xb15316i1700zp0yi0xs0yi0xs0yi0xs0yi0xs0yi0xs16o0zs16o0zs16o0zs16o0zs16g0z519m0z50fv0f60fv0f60fv0f60fv0f60fv0f61cz0ub0x40f614h0wp0ws0w60f60w60f60w60lz0wq0op0yc0f617a0z417a0z417a0z417a0z417s0yr17s0yr17s0yr1ls1kb1190mw1190mw1190mw1150vk1150vk1150vk1150vk11s0kh11s0oi11s0kh15r0z515r0z515r0z515r0z515r0z515r0z51mj1c21530xv15311g0we11g0we11g0we0jb0gd0gg17a0gi0xb0xb0xb0j40x40iq14l15y0ik18p1gn0q21ft1hv1g30h915y11l0x415o0yi11g16g17s0fv14h15y1gr17a12t17s16g10s13f11s1531ep15h1eq18d0fv1531370vj0z50h90z41370yj0xr0y80vj0u60z50zm0h90ws12g0zc0yp0vs0yr1010zp0x910s0sm0z418o0xy1ar1b60h90z40yr0z41b614h11r11p13z1fp13z1611fr0ws0xx0uj16z11r0xb0f617s0qz0qz1601gr0zp1601601600yi0yi1co0x41601150fv0fv0x41rj1s31et14h17a12w15c15y10s11l0x41cx0yi1nr10g17a17a14h16n1gr16g17s16g10s16011s12w1bh15h1771461jz1lf1b31fx11l1601n21190wu0yj0wp0pl1150xs1cn0tb0z50z50ws0y319t0z10yr0yn0zp0xb0sm0xv17c0wp10r0xz1ck1e01291980x90xb1d30wn0xs0xs0z50pl0xb0vk0f60f60f61hi1gc0z50ws0z50xv0yn0ox0ox10s0sg1kw1kw0vc0qv0h70h70fz0h70ts0te0s60te0wb0wb0rt0rt0ik1151jp0ik26j2rm0f40u51960f40u51960ub0o30o312s1ia1eu1ia1eu12g12g12g",
    700: "0dh0j80vd10x1191ls1280ja0lg0lg0vt12m0j00qm0j00m312d0oj0zt10q12h0ze10y0x311110y0j00ji12m12m12m0vu1lt16h11n1631530yk0xe16p16i0fz0x914x0w61h017d17u10v18711d11911z15n16h1n116015l11s0lg0m30lg0rp0r30ks0x10zv0xh0zv0xw0mn0zy0zf0ff0ff0x00ff1fx0zf0yw0zv0zv0n60vv0ku0zf0y41cd0x00y90wl0qo0l50qo12m0dh0j80xh10c17c0wg0ja0wc0zf1g00qe11k12m11o0oz0q512m0q60qu0ks0zq0y20j00kq0iy0rv11k1c41e51gc0vu16h16h16h16h16h16h1m61630yk0yk0yk0yk0fz0fz0fz0fz17817d17u17u17u17u17u12m17u15n15n15n15n15l12211e0x10x10x10x10x10x11fs0xh0xw0xw0xw0xw0ff0ff0ff0ff0y30zf0yw0yw0yw0yw0yw12m0yw0zf0zf0zf0zf0y90zv0y916h0x116h0x116h0x11630xh1630xh1630xh1630xh15316x1780zv0yk0xw0yk0xw0yk0xw0yk0xw0yk0xw16p0zy16p0zy16p0zy16p0zy16i0zf19x0zf0fz0ff0fz0ff0fz0ff0fz0ff0fz0ff1d70uv0x90ff14x0x00xd0w60ff0w60ff0w60mh0wv0pk0yg0ff17d0zf17d0zf17d0zf17d0zf17u0yw17u0yw17u0yw1lw1k711d0n611d0n611d0n61190vv1190vv1190vv1190vv11z0ku11z0p411z0ku15n0zf15n0zf15n0zf15n0zf15n0zf15n0zf1n11cd15l0y915l11s0wl11s0wl11s0wl0jm0gm0go17d0gq0xh0xh0xh0ji0x90je15116h0j01981h60qn1gd1in1gm0hl16h11n0x51690yk11s16i17u0fz14x16h1h017d13417u16i10v13g11z15l1et1601ev18h0fz15l13h0vq0zf0hl0ze13h0yo0y40yf0vq0ue0zf0zo0hl0xd12u0zq0yz0vr0yw10d0zv0xf1110t10ze19f0y91b81bf0hl0ze0yw0ze1bf14x12312014c1gg14c16n1gr0xd0xz0uk17c1230xh0ff17u0r80r81631h00zv1631631630yk0yk1cu0x51631190fz0fz0x91rl1s31ew14x17d13i15d16h10t11n0x51de0yk1os10q17d17d14x16n1h016i17u16i10v16311z13i1bj16017n1491k91lu1b81g911p1631nb11d0x10yo0wt0pr11l0xw1d60tj0zf0zf0x60yd1a70zc0yw0yw0zv0xh0t10y91870x011b0y71cx1ei12l19w0xk0xh1dc0wv0xw0xw0zf0pr0xh0vv0ff0ff0ff1hp1gl0zf0x60zf0y90yw0pt0pt10s0sg1kw1kw0w60r30ho0ho0gf0ho0uq0u90t00u90x60x60qz0qz0j01201l00j02722sc0fm0v51ao0fm0v51ao0v70oj0oj12y1ia1fi1ia1fi12m12m12m",
    800: "0cg0ke0xd11a11k1mk12v0k60lr0lr0x51300k20qt0k20mr13d0p410a11d13610211n0xg11s11n0k20kh1301301300wy1my17s11t16b1540yp0xb16t16l0g90xk1600w61ho17k17z11318h11o11k12i15d17s1o817b16s12m0lr0mr0lr0s20rk0lr0xh10a0xv10a0y60nb10d1050g40g40xq0g41gr1050z810a10a0nw0wm0lq1050z01d30xt0z60x20ro0m20ro1300cg0ke0xv10z1860wx0kv0wc1051g00qm13c13011l0o30q81300ql0rg0lr10q0xz0k20my0jn0s113c1d41ex1h70wy17s17s17s17s17s17s1mv16b0yp0yp0yp0yp0g90g90g90g917t17k17z17z17z17z17z13017z15d15d15d15d16s12u12d0xh0xh0xh0xh0xh0xh1fm0xv0y60y60y60y60g40g40g40g40yh1050z80z80z80z80z81300z81051051051050z610a0z617s0xh17s0xh17s0xh16b0xv16b0xv16b0xv16b0xv15417y17t10a0yp0y60yp0y60yp0y60yp0y60yp0y616t10d16t10d16t10d16t10d16l1051am1050g90g40g90g40g90g40g90g40g90g41dt0w70xk0g41600xq0ys0w60g40w60g40w60ns0x80rm0yo0g417k10517k10517k10517k10517z0z817z0z817z0z81m71jy11o0nw11o0nw11o0nw11k0wm11k0wm11k0wm11k0wm12i0lq12i0qm12i0lq15d10515d10515d10515d10515d10515d1051o81d316s0z616s12m0x212m0x212m0x20kb0h70h817k0hc0xv0xv0xv0kh0xk0l116617s0k21aj1if0s31hq1kj1hv0id17s11t0x817o0yp12m16l17z0g916017s1ho17k13u17z16l11313k12i16s1f417b1f718r0g916s1460w71050id1041460z00z00yx0w70uz1050zt0id0ys13t10q0zm0vr0z811510a0xu11o0u21041b70z11cg1c40id1040z81041c416012x12s1581i81581851j90ys0y20un18612x0xv0g417z0rw0rw16b1ho10a16b16b16b0yp0yp1d80x816b11k0g90g90xk1ro1s51f516017k15115h17s10t11t0x81ej0yp1rb11d17k17k16016p1ho16l17z16l11316b12i1511bp17b18q14f1kz1mv1bk1h312016b1nz11o0xh0z20x30q512p0y61ef0u41051050y50z11b61040z80zh10a0xv0u20z61aa0xt12o0yr1ds1fo13e1bi0ya0xv1dw0xf0y60y61050q50xv0wm0g40g40g41i51h91050y51050z60zh0s10s110q0sg1kw1kw0y60rn0iu0iu0hi0iu0x10we0v20we0za0za0ox0ox0k21441o70k228b2u20gt0xk1ec0gt0xk1ec0xd0pm0pm13d1ia1h51ia1h5130130130"
  };

  // src/hmiBuilder/drawing/measure.ts
  var WIDTHS = Object.entries(FONT_WIDTHS).map(([weight, packed]) => {
    const widths = /* @__PURE__ */ new Map();
    Array.from(FONT_CHARS).forEach(
      (letter, index) => widths.set(letter, parseInt(packed.slice(index * 3, index * 3 + 3), 36))
    );
    return [Number(weight), widths];
  });
  function textWidth(content, size, weight = 400) {
    const widths = WIDTHS.reduce(
      (nearest, entry) => Math.abs(entry[0] - weight) < Math.abs(nearest[0] - weight) ? entry : nearest
    )[1];
    let units = 0;
    for (const letter of content) units += widths.get(letter) ?? FONT_UNITS;
    return units / FONT_UNITS * size;
  }

  // src/hmiBuilder/drawing/speech.ts
  var ENGLISH = { texts: {}, language: "en" };
  var current = ENGLISH;
  var base = ENGLISH;
  function setSpeech(texts, language2) {
    base = { texts, language: language2 };
    current = base;
  }
  function say(word, values = {}) {
    return spoken(fill(current.texts[word] ?? word, values));
  }
  function translate(text2) {
    return spoken(current.texts[text2] ?? text2);
  }
  function spoken(text2) {
    return heard(current.longest ? lengthen(text2) : text2);
  }
  function lengthen(text2) {
    if (text2.trim() === "") return text2;
    const goal = textWidth(text2, 14) * 1.4;
    let longer = `${text2} `;
    while (textWidth(longer, 14) < goal) longer += "\xB7";
    return longer;
  }
  function displaySpeech() {
    return { texts: base.texts, language: base.language };
  }
  function language() {
    return current.language;
  }
  function fill(text2, values) {
    return text2.replace(/\{([A-Za-z]+)\}/g, (whole, name) => name in values ? String(values[name]) : whole);
  }

  // src/hmiBuilder/display/alarms.ts
  var MAX_RECORDS = 1e3;
  var PRIORITIES = ["high", "medium", "low"];
  async function loadRecords(host2, model, view) {
    if (!model.object.some((object) => object.type === "event-log")) return;
    try {
      const answer = await host2.fetchJson("/api/hmi/alarms");
      if (answer.status === 200 && answer.body && typeof answer.body === "object")
        view.receive(answer.body);
    } catch {
    }
  }
  var AlarmView = class {
    constructor(sounds) {
      this.records = [];
      /** Each alarm list's and event log's tab and page, by object id. */
      this.views = {};
      this.sound = new AlarmSound(sounds);
    }
    /** Silences the High alarms sounding now, without acknowledging them. */
    silence() {
      this.sound.silence(this.summary);
    }
    /** What the drawing gets for alarm lists and event logs. */
    frame(stale) {
      return { summary: this.summary, records: this.records, stale, views: this.views };
    }
    receive(message) {
      if (message.alarms) {
        this.summary = message.alarms;
        this.sound.update(this.summary);
      }
      if (!Array.isArray(message.events ?? [])) return;
      for (const record of message.events ?? []) {
        if (!this.records.some((known2) => known2.seq === record.seq)) this.records.push(record);
      }
      this.records.sort((a, b) => a.seq - b.seq);
      if (this.records.length > MAX_RECORDS) this.records.splice(0, this.records.length - MAX_RECORDS);
    }
    /** Fills the top bar's alarm area; while the data is stale it is marked and nothing blinks. */
    drawTopBar(area, stale, painting) {
      const summary = this.summary;
      const key = JSON.stringify([summary ?? null, stale, this.sound.sounding]);
      if (key === this.drawn) return;
      this.drawn = key;
      area.silence.hidden = !this.sound.sounding;
      const annunciating = summary?.active ?? [];
      const top = PRIORITIES.find((priority) => annunciating.some((alarm) => alarm.priority === priority));
      const document2 = area.count.ownerDocument;
      const label = annunciating.length === 0 ? say("No alarms") : annunciating.length === 1 ? say("1 alarm") : say("{count} alarms", { count: annunciating.length });
      area.count.replaceChildren(
        ...top ? [markOf(document2, painting, top)] : [],
        spanOf(document2, "hmi-alarm-label", label)
      );
      flag(area.count, "data-priority", top);
      flag(area.count, "data-blink", !stale && (summary?.unacknowledged ?? 0) > 0 ? "true" : void 0);
      flag(area.count, "data-stale", stale ? "true" : void 0);
      const newest = summary?.newest;
      if (newest) {
        area.newest.replaceChildren(
          markOf(document2, painting, newest.priority),
          spanOf(document2, "hmi-alarm-message", translate(newest.message || newest.path)),
          spanOf(document2, "hmi-alarm-time", painting.time(newest.since_ms))
        );
        area.newest.title = newest.path;
      } else {
        area.newest.replaceChildren();
        area.newest.removeAttribute("title");
      }
      flag(area.newest, "data-priority", newest?.priority);
      flag(area.newest, "data-stale", stale && newest ? "true" : void 0);
      const hidden = [
        [summary?.shelved.length ?? 0, "{count} shelved"],
        [summary?.suppressed.length ?? 0, "{count} suppressed"],
        [summary?.out_of_service.length ?? 0, "{count} out of service"]
      ].filter(([count]) => count > 0).map(([count, words]) => say(words, { count }));
      area.hidden.textContent = hidden.join(" \xB7 ");
    }
  };
  function markOf(document2, painting, priority) {
    const mark = document2.createElement("span");
    mark.className = "hmi-alarm-mark";
    mark.innerHTML = painting.mark(priority);
    return mark;
  }
  function spanOf(document2, className, content) {
    const span = document2.createElement("span");
    span.className = className;
    span.textContent = content;
    return span;
  }
  function flag(element2, name, value) {
    if (value === void 0) element2.removeAttribute(name);
    else element2.setAttribute(name, value);
  }

  // src/hmiBuilder/drawing/format.ts
  function formatValue(value, object) {
    if (value === null || value === void 0) return "?";
    if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
    if (typeof value === "string") return durationWords(value, object.decimals) ?? value;
    if (typeof value !== "number" || !Number.isFinite(value)) return String(value);
    if (object.decimals !== void 0) return localNumber(value, object.decimals, object.decimals);
    return localNumber(Number(value.toFixed(3)), 0, 3);
  }
  var DURATION = /^T#(-)?(?:(\d+)d)?(?:(\d+)h)?(?:(\d+)m(?!s))?(?:(\d+)s)?(?:(\d+)ms)?(?:(\d+)us)?(?:(\d+)ns)?$/;
  function durationWords(literal, decimals) {
    const match2 = DURATION.exec(literal);
    if (!match2 || literal === "T#" || literal === "T#-") return void 0;
    const [, minus, days, hours, minutes, seconds, millis, micros, nanos] = match2;
    if (decimals !== void 0) {
      const scale = 10 ** decimals;
      const total = Math.round(
        (Number(days ?? 0) * 86400 + Number(hours ?? 0) * 3600 + Number(minutes ?? 0) * 60 + Number(seconds ?? 0) + Number(millis ?? 0) / 1e3 + Number(micros ?? 0) / 1e6 + Number(nanos ?? 0) / 1e9) * scale
      ) / scale;
      const whole = Math.floor(total);
      const rounded = [];
      if (whole >= 86400) rounded.push(`${Math.floor(whole / 86400)} d`);
      if (whole % 86400 >= 3600) rounded.push(`${Math.floor(whole % 86400 / 3600)} h`);
      if (whole % 3600 >= 60) rounded.push(`${Math.floor(whole % 3600 / 60)} min`);
      const rest = total - Math.floor(total / 60) * 60;
      if (rest > 0 || rounded.length === 0) rounded.push(`${localNumber(rest, decimals, decimals)} s`);
      return `${minus ?? ""}${rounded.join(" ")}`;
    }
    const parts = [];
    if (days) parts.push(`${Number(days)} d`);
    if (hours) parts.push(`${Number(hours)} h`);
    if (minutes) parts.push(`${Number(minutes)} min`);
    const second = Number(seconds ?? 0) + Number(millis ?? 0) / 1e3 + Number(micros ?? 0) / 1e6 + Number(nanos ?? 0) / 1e9;
    if (second > 0 || parts.length === 0) parts.push(`${localNumber(Number(second.toFixed(3)), 0, 3)} s`);
    return `${minus ?? ""}${parts.join(" ")}`;
  }
  var NUMBER_FORMATS = /* @__PURE__ */ new Map();
  function localNumber(value, fewest, most) {
    const spoken2 = language();
    if (spoken2 === "en") return fewest === most ? value.toFixed(fewest) : value.toString();
    const key = `${spoken2}/${fewest}/${most}`;
    let format = NUMBER_FORMATS.get(key);
    if (!format) {
      format = new Intl.NumberFormat(spoken2, {
        minimumFractionDigits: fewest,
        maximumFractionDigits: most,
        useGrouping: false
      });
      NUMBER_FORMATS.set(key, format);
    }
    return format.format(value);
  }
  function timeOfDay(ms) {
    const time = new Date(ms);
    const two = (value) => String(value).padStart(2, "0");
    return `${two(time.getHours())}:${two(time.getMinutes())}:${two(time.getSeconds())}`;
  }

  // src/hmiBuilder/display/icons.ts
  var PATHS = {
    overview: "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
    process: "M3 7h9a4 4 0 0 1 4 4v9 M3 16h5 M16 4v3",
    trend: "M3 18l6-6 4 4 8-9 M3 21h18",
    alarm: "M6 16V11a6 6 0 0 1 12 0v5l2 2H4z M10 20a2 2 0 0 0 4 0",
    settings: "M12 8.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7z M12 2v3 M12 19v3 M2 12h3 M19 12h3 M4.9 4.9l2.1 2.1 M17 17l2.1 2.1 M4.9 19.1L7 17 M17 7l2.1-2.1",
    pump: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16z M9 8l7 4-7 4z",
    valve: "M3 7l9 5 9-5v10l-9-5-9 5z M12 12V4 M9 4h6",
    tank: "M5 5a7 2 0 0 1 14 0v14a7 2 0 0 1-14 0z M5 13a7 2 0 0 0 14 0",
    // The language button's globe (§14.5).
    globe: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M3 12h18 M12 3a14 14 0 0 1 0 18 M12 3a14 14 0 0 0 0 18",
    motor: "M4 7h12v10H4z M16 10h4v4h-4 M7 4v3 M13 4v3",
    heat: "M12 3c3 4 5 6 5 10a5 5 0 0 1-10 0c0-2 1-4 2-5 0 2 1 3 2 3 0-3 0-5 1-8z",
    flow: "M3 9c3-3 6 3 9 0s6 3 9 0 M3 15c3-3 6 3 9 0s6 3 9 0",
    power: "M13 2L5 14h6l-1 8 8-12h-6z",
    list: "M8 6h13 M8 12h13 M8 18h13 M3 6h1 M3 12h1 M3 18h1",
    info: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 11v6 M12 7.5v.5"
  };
  function icon(document2, name) {
    const path = name === void 0 ? void 0 : PATHS[name];
    if (!path || !name) return void 0;
    const namespace = "http://www.w3.org/2000/svg";
    const svg = document2.createElementNS(namespace, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("width", "20");
    svg.setAttribute("height", "20");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("data-icon", name);
    const drawn = document2.createElementNS(namespace, "path");
    drawn.setAttribute("d", path);
    drawn.setAttribute("fill", "none");
    drawn.setAttribute("stroke", "currentColor");
    drawn.setAttribute("stroke-width", "1.7");
    drawn.setAttribute("stroke-linecap", "round");
    drawn.setAttribute("stroke-linejoin", "round");
    svg.append(drawn);
    return svg;
  }

  // src/hmiBuilder/display/frame.ts
  var FRAME_CSS = `
.hmi-frame { position: fixed; inset: 0; display: flex; flex-direction: column; background: var(--hmi-page); }
.hmi-topbar { box-sizing: border-box; height: 56px; flex: none; display: flex; align-items: center; gap: 18px; padding: 0 18px; background: var(--hmi-surface); border-bottom: 1px solid var(--hmi-border); font: 500 14px var(--hmi-font); color: var(--hmi-text); }
.hmi-topbar .hmi-brand { display: flex; align-items: center; gap: 10px; font: 650 17px var(--hmi-font); letter-spacing: -0.2px; white-space: nowrap; }
.hmi-topbar .hmi-brand img { height: 30px; width: auto; }
.hmi-topbar .hmi-page-title { flex: 1; text-align: center; color: var(--hmi-muted); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.hmi-topbar .hmi-spacer { flex: 1; }
.hmi-language { position: relative; }
.hmi-language-button { display: inline-flex; align-items: center; gap: 8px; height: 44px; padding: 0 14px 0 10px; border-radius: 999px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 14px var(--hmi-font); cursor: pointer; }
.hmi-language-button svg { color: var(--hmi-muted); }
.hmi-language-menu { position: fixed; z-index: 30; display: flex; flex-direction: column; min-width: 200px; padding: 6px; border-radius: 14px; border: 1px solid var(--hmi-border); background: var(--hmi-surface); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35); }
.hmi-language-menu[hidden] { display: none; }
.hmi-language-menu button { min-height: 44px; padding: 0 14px; border: 0; border-radius: 10px; background: transparent; color: var(--hmi-text); font: 500 15px var(--hmi-font); text-align: left; cursor: pointer; }
.hmi-language-menu button:hover { background: var(--hmi-raised); }
.hmi-language-menu button[aria-checked="true"] { color: var(--hmi-accent); font-weight: 650; }
.hmi-corner { position: fixed; top: 68px; right: 16px; z-index: 12; display: flex; align-items: center; gap: 8px; }
.hmi-topbar .hmi-clock { font: 600 15px var(--hmi-font); font-variant-numeric: tabular-nums; color: var(--hmi-text); }
.hmi-topbar .hmi-alarms { display: flex; align-items: center; gap: 14px; min-width: 0; }
.hmi-topbar .hmi-alarm-count { display: inline-flex; align-items: center; gap: 8px; padding: 4px 12px; border-radius: 999px; border: 1px solid var(--hmi-border); color: var(--hmi-muted); font-weight: 650; white-space: nowrap; }
.hmi-topbar .hmi-alarm-count[data-priority="high"] { color: var(--hmi-alarm-high); border-color: var(--hmi-alarm-high); background: color-mix(in srgb, var(--hmi-alarm-high) 14%, transparent); }
.hmi-topbar .hmi-alarm-count[data-priority="medium"] { color: var(--hmi-alarm-medium); border-color: var(--hmi-alarm-medium); background: color-mix(in srgb, var(--hmi-alarm-medium) 14%, transparent); }
.hmi-topbar .hmi-alarm-count[data-priority="low"] { color: var(--hmi-alarm-low); border-color: var(--hmi-alarm-low); background: color-mix(in srgb, var(--hmi-alarm-low) 12%, transparent); }
.hmi-topbar .hmi-alarm-count[data-blink="true"] .hmi-alarm-mark { animation: hmi-blink 1s step-end infinite; }
.hmi-topbar [data-stale="true"] { outline: 2px dashed var(--hmi-stale); outline-offset: 2px; opacity: 0.85; }
.hmi-topbar .hmi-alarm-newest { display: inline-flex; align-items: center; gap: 8px; min-width: 0; max-width: 420px; border-radius: 6px; }
.hmi-topbar .hmi-alarm-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; color: var(--hmi-text); }
.hmi-topbar .hmi-alarm-time { color: var(--hmi-muted); font-variant-numeric: tabular-nums; }
.hmi-topbar .hmi-alarm-hidden { color: var(--hmi-muted); font-size: 13px; white-space: nowrap; }
.hmi-topbar .hmi-alarm-silence { min-width: 44px; min-height: 44px; padding: 4px 14px; border-radius: 999px; border: 1px solid var(--hmi-alarm-high); background: color-mix(in srgb, var(--hmi-alarm-high) 18%, transparent); color: var(--hmi-text); font: 650 14px var(--hmi-font); cursor: pointer; }
.hmi-topbar .hmi-alarm-silence[hidden] { display: none; }
.hmi-topbar .hmi-theme-switch { flex: none; width: 44px; height: 44px; border-radius: 22px; border: 1px solid var(--hmi-border); background: transparent; color: var(--hmi-text); font-size: 17px; line-height: 1; cursor: pointer; }
.hmi-topbar .hmi-alarm-mark { display: inline-flex; flex: none; }
.hmi-simulation { flex: none; display: inline-flex; align-items: center; gap: 8px; box-sizing: border-box; height: 32px; padding: 0 12px; border-radius: 999px; border: 1px solid var(--hmi-accent-3); background: color-mix(in srgb, var(--hmi-accent-3) 18%, transparent); color: var(--hmi-text); font: 750 12px var(--hmi-font); letter-spacing: 0.6px; white-space: nowrap; font-variant-numeric: tabular-nums; }
.hmi-simulation[hidden] { display: none; }
.hmi-simulation::before { content: ""; flex: none; width: 7px; height: 7px; border-radius: 50%; background: var(--hmi-accent-3); }
@media (max-width: 720px) { .hmi-simulation { gap: 6px; height: 28px; padding: 0 9px; font-size: 11px; } }

.hmi-session.hmi-session.hmi-session-inline { position: static; box-shadow: none; padding: 3px 4px 3px 12px; }
.hmi-with-top-bar .hmi-session:not(.hmi-session-inline) { top: 68px; }
.hmi-body { flex: 1; display: flex; min-height: 0; }
.hmi-side { box-sizing: border-box; width: 232px; flex: none; overflow-y: auto; padding: 14px 10px; display: flex; flex-direction: column; gap: 2px; background: var(--hmi-surface); border-right: 1px solid var(--hmi-border); }
.hmi-side[data-position="right"] { order: 2; border-right: none; border-left: 1px solid var(--hmi-border); }
.hmi-side .hmi-group { margin: 14px 12px 6px; font: 650 11px var(--hmi-font); letter-spacing: 0.8px; text-transform: uppercase; color: var(--hmi-muted); }
.hmi-nav-item { display: flex; align-items: center; gap: 12px; min-height: 44px; padding: 0 12px; border-radius: 10px; text-decoration: none; font: 550 15px var(--hmi-font); color: var(--hmi-muted); }
.hmi-nav-item:hover { background: var(--hmi-raised); color: var(--hmi-text); }
.hmi-nav-item[aria-current="page"] { background: color-mix(in srgb, var(--hmi-accent) 14%, transparent); color: var(--hmi-accent); font-weight: 700; box-shadow: inset 3px 0 0 var(--hmi-accent); }
.hmi-tabs { box-sizing: border-box; height: 48px; flex: none; display: flex; align-items: stretch; gap: 4px; padding: 0 12px; background: var(--hmi-surface); border-bottom: 1px solid var(--hmi-border); overflow-x: auto; }
.hmi-tabs .hmi-nav-item { border-radius: 0; box-shadow: none; min-height: 0; }
.hmi-tabs .hmi-nav-item[aria-current="page"] { background: transparent; box-shadow: inset 0 -3px 0 var(--hmi-accent); }
.hmi-tabs .hmi-divider { width: 1px; margin: 12px 6px; background: var(--hmi-border); }
.hmi-main { flex: 1; min-width: 0; min-height: 0; position: relative; }
`;
  function buildFrame(container, data, pageName, pageTitle, language2, asset = assetUrl, signal) {
    const document2 = container.ownerDocument;
    const frame = document2.createElement("div");
    frame.className = "hmi-frame";
    const topBar = data.screen?.top_bar;
    if (topBar) container.classList.add("hmi-with-top-bar");
    const bar = topBar ? buildTopBar(document2, topBar, pageTitle, language2, container, asset, signal) : void 0;
    if (bar) frame.append(bar.element);
    const body = document2.createElement("div");
    body.className = "hmi-body";
    const main = document2.createElement("main");
    main.className = "hmi-main";
    const navigation = buildNavigation(document2, data, pageName);
    if (navigation?.getAttribute("data-style") === "tabs") frame.append(navigation);
    else if (navigation) body.append(navigation);
    body.append(main);
    frame.append(body);
    const simulation = bar?.simulation ?? buildSimulation(document2);
    if (!bar) {
      const corner = document2.createElement("div");
      corner.className = "hmi-corner";
      corner.append(simulation);
      if (language2) corner.append(buildLanguage(document2, language2, container, signal));
      frame.append(corner);
    }
    container.append(frame);
    const clock = bar?.clock;
    const tick = (nowMs) => {
      if (clock) clock.textContent = timeOfDay(nowMs);
    };
    return { main, sessionSlot: bar?.sessionSlot, tick, alarms: bar?.alarms, topBar: bar?.element, simulation };
  }
  function buildTopBar(document2, model, pageTitle, language2, layer, asset = assetUrl, signal) {
    const bar = document2.createElement("header");
    bar.className = "hmi-topbar";
    bar.setAttribute("data-part", "top-bar");
    const brand = document2.createElement("div");
    brand.className = "hmi-brand";
    if (model.logo) {
      const logo = document2.createElement("img");
      logo.src = asset(model.logo);
      logo.alt = "";
      brand.append(logo);
    }
    const title = document2.createElement("span");
    title.textContent = translate(model.title);
    brand.append(title);
    bar.append(brand);
    const simulation = buildSimulation(document2);
    bar.append(simulation);
    if (model.page_title) {
      const page = document2.createElement("div");
      page.className = "hmi-page-title";
      page.setAttribute("data-part", "page-title");
      page.textContent = pageTitle;
      bar.append(page);
    } else {
      const spacer = document2.createElement("div");
      spacer.className = "hmi-spacer";
      bar.append(spacer);
    }
    let alarms;
    if (model.alarms) {
      const area = document2.createElement("div");
      area.className = "hmi-alarms";
      area.setAttribute("data-part", "alarms");
      const part = (name, className) => {
        const element2 = document2.createElement("div");
        element2.className = className;
        element2.setAttribute("data-part", name);
        area.append(element2);
        return element2;
      };
      const silence = document2.createElement("button");
      silence.type = "button";
      silence.className = "hmi-alarm-silence";
      silence.setAttribute("data-action", "silence");
      silence.textContent = say("Silence");
      silence.hidden = true;
      alarms = {
        count: part("alarm-count", "hmi-alarm-count"),
        newest: part("alarm-newest", "hmi-alarm-newest"),
        hidden: part("alarm-hidden", "hmi-alarm-hidden"),
        silence
      };
      area.insertBefore(silence, alarms.newest);
      alarms.count.textContent = say("No alarms");
      bar.append(area);
    }
    let clock;
    if (model.clock) {
      clock = document2.createElement("div");
      clock.className = "hmi-clock";
      clock.setAttribute("data-part", "clock");
      bar.append(clock);
    }
    if (language2 && layer) bar.append(buildLanguage(document2, language2, layer, signal));
    let sessionSlot;
    if (model.user) {
      sessionSlot = document2.createElement("div");
      sessionSlot.className = "hmi-session-slot";
      bar.append(sessionSlot);
    }
    return { element: bar, clock, sessionSlot, alarms, simulation };
  }
  function buildSimulation(document2) {
    const badge = document2.createElement("div");
    badge.className = "hmi-simulation";
    badge.setAttribute("data-part", "simulation");
    badge.hidden = true;
    const label = document2.createElement("span");
    label.textContent = say("SIMULATION");
    const speed = document2.createElement("span");
    speed.setAttribute("data-part", "simulation-speed");
    speed.hidden = true;
    badge.append(label, " ", speed);
    return badge;
  }
  function showSimulation(badge, status) {
    badge.hidden = status?.enabled !== true;
    const text2 = status?.enabled === true ? simulationSpeed(status) : "";
    const speed = badge.querySelector('[data-part="simulation-speed"]');
    if (!speed) return;
    speed.textContent = text2;
    speed.hidden = text2 === "";
  }
  function simulationSpeed(status) {
    const configured = status.speed_configured;
    if (typeof configured !== "number") return "";
    const achieved = status.speed_achieved;
    if (typeof achieved === "string") {
      return say("{achieved} of {configured}", { achieved: `\xD7${achieved}`, configured: `\xD7${configured}` });
    }
    return `\xD7${configured}`;
  }
  function buildLanguage(document2, choice, layer, signal) {
    const box = document2.createElement("div");
    box.className = "hmi-language";
    box.setAttribute("data-part", "language");
    const button2 = document2.createElement("button");
    button2.type = "button";
    button2.className = "hmi-language-button";
    button2.setAttribute("data-action", "language");
    button2.setAttribute("aria-haspopup", "menu");
    button2.setAttribute("aria-expanded", "false");
    button2.setAttribute("aria-label", say("Language"));
    const globe = icon(document2, "globe");
    if (globe) button2.append(globe);
    const name = document2.createElement("span");
    name.textContent = choice.languages.find((entry) => entry.tag === choice.current)?.name ?? choice.current;
    button2.append(name);
    const menu = document2.createElement("div");
    menu.className = "hmi-language-menu";
    menu.setAttribute("data-part", "language-menu");
    menu.setAttribute("role", "menu");
    menu.hidden = true;
    for (const entry of choice.languages) {
      const item = document2.createElement("button");
      item.type = "button";
      item.setAttribute("role", "menuitemradio");
      item.setAttribute("data-language", entry.tag);
      item.setAttribute("aria-checked", String(entry.tag === choice.current));
      item.setAttribute("lang", entry.tag);
      item.textContent = entry.name;
      item.addEventListener("click", () => {
        menu.hidden = true;
        if (entry.tag !== choice.current) choice.choose(entry.tag);
      });
      menu.append(item);
    }
    const show = (open) => {
      if (open) {
        const at = button2.getBoundingClientRect();
        const width = document2.defaultView?.innerWidth ?? at.right;
        menu.style.top = `${Math.round(at.bottom + 6)}px`;
        menu.style.right = `${Math.max(8, Math.round(width - at.right))}px`;
      }
      menu.hidden = !open;
      button2.setAttribute("aria-expanded", String(open));
    };
    button2.addEventListener("click", () => show(menu.hidden !== false));
    const outside = (event) => {
      const target = event.target;
      if (!menu.hidden && target && !menu.contains(target) && !button2.contains(target)) show(false);
    };
    const escape = (event) => {
      if (event.key === "Escape" && !menu.hidden) show(false);
    };
    document2.addEventListener("click", outside);
    document2.addEventListener("keydown", escape);
    signal?.addEventListener(
      "abort",
      () => {
        document2.removeEventListener("click", outside);
        document2.removeEventListener("keydown", escape);
      },
      { once: true }
    );
    box.append(button2);
    layer.append(menu);
    return box;
  }
  function buildNavigation(document2, data, pageName) {
    const style = data.screen?.navigation?.style ?? "side";
    const entries = data.navigation ?? [];
    if (style === "none" || entries.length < 2) return void 0;
    const ordered = [...entries.filter((entry) => !entry.group), ...entries.filter((entry) => entry.group)];
    const navigation = document2.createElement("nav");
    navigation.className = style === "tabs" ? "hmi-tabs" : "hmi-side";
    navigation.setAttribute("data-part", "navigation");
    navigation.setAttribute("data-style", style);
    navigation.setAttribute("data-position", data.screen?.navigation?.position ?? "left");
    navigation.setAttribute("aria-label", say("Pages"));
    let group;
    for (const entry of ordered) {
      if (entry.group !== group) {
        group = entry.group;
        navigation.append(groupMark(document2, style, group));
      }
      navigation.append(link(document2, entry, entry.name === pageName));
    }
    return navigation;
  }
  function groupMark(document2, style, group) {
    const mark = document2.createElement("div");
    if (style === "tabs") {
      mark.className = "hmi-divider";
    } else {
      mark.className = "hmi-group";
      mark.setAttribute("data-part", "group");
      mark.textContent = group !== void 0 ? translate(group) : "";
    }
    return mark;
  }
  function link(document2, entry, current2) {
    const anchor = document2.createElement("a");
    anchor.className = "hmi-nav-item";
    anchor.href = `/hmi/${encodeURIComponent(entry.name)}`;
    if (current2) anchor.setAttribute("aria-current", "page");
    const drawn = icon(document2, entry.icon);
    if (drawn) anchor.append(drawn);
    const title = document2.createElement("span");
    title.textContent = translate(entry.title);
    anchor.append(title);
    return anchor;
  }
  function assetUrl(path) {
    return path.startsWith("hmi/assets/") ? `/hmi/project-assets/${path.slice("hmi/assets/".length)}` : path;
  }

  // src/hmiBuilder/display/liveValues.ts
  var DEFAULT_PERIOD_MS = 250;
  var FRESH_PERIODS = 3;
  var MAX_SAMPLES = 2e4;
  var LONG_SPAN_MS = 6e5;
  var LiveValueStore = class {
    constructor(options = {}) {
      this.values = /* @__PURE__ */ new Map();
      this.history = /* @__PURE__ */ new Map();
      this.periodMs = DEFAULT_PERIOD_MS;
      /** The paths the running program does not have, from the newest update (§16.3). */
      this.missing = [];
      this.trendSpansMs = options.trendSpansMs ?? {};
    }
    receive(message, atMs) {
      this.disconnectedAtMs = void 0;
      if (message.period_ms !== void 0 && message.period_ms > 0) this.periodMs = message.period_ms;
      if (message.cycle === void 0 || message.values === void 0) return;
      this.lastMessageMs = atMs;
      if (message.missing !== void 0) this.missing = message.missing;
      const advanced = this.cycle === void 0 || message.cycle !== this.cycle;
      this.cycle = message.cycle;
      if (advanced) this.lastAdvanceMs = atMs;
      for (const [path, wire] of Object.entries(message.values)) this.values.set(path, wire);
      if (!advanced) return;
      for (const path of Object.keys(this.trendSpansMs)) {
        const wire = this.values.get(path);
        if (wire !== void 0) this.sample(path, wire, atMs);
      }
    }
    disconnected(atMs) {
      this.disconnectedAtMs = atMs;
    }
    frame(nowMs) {
      const staleMs = this.staleness(nowMs);
      const values = {};
      for (const [path, wire] of this.values) {
        if (wire === null) {
          values[path] = { value: null, state: "unknown" };
          continue;
        }
        const age = Math.max(staleMs ?? -1, wire.staleMs ?? -1);
        values[path] = {
          value: wire.value,
          state: "confirmed",
          ...wire.presentation ? { presentation: wire.presentation } : {},
          ...wire.lowerBounds ? { lowerBounds: wire.lowerBounds } : {},
          ...age >= 0 ? { staleMs: age } : {}
        };
      }
      return { values, history: Object.fromEntries(this.history), nowMs, missing: this.missing };
    }
    /** Whether the stream is closed or its updates are stale (§5.3, item 2), as alarms show it. */
    isStale(nowMs) {
      return this.disconnectedAtMs !== void 0 || this.staleness(nowMs) !== void 0;
    }
    /** The age of the values when they count as stale, otherwise undefined. */
    staleness(nowMs) {
      if (this.lastAdvanceMs === void 0 || this.lastMessageMs === void 0) return void 0;
      const limit2 = FRESH_PERIODS * this.periodMs;
      const stalled = nowMs - this.lastAdvanceMs > limit2 || nowMs - this.lastMessageMs > limit2;
      return this.disconnectedAtMs !== void 0 || stalled ? nowMs - this.lastAdvanceMs : void 0;
    }
    /**
     * Takes a trend path's samples from the runtime's historian (§5.7, item 1): those older than
     * the samples the stream already gave, then the span rule as for the stream's own samples.
     */
    seed(path, samples) {
      const spanMs = this.trendSpansMs[path];
      if (spanMs === void 0) return;
      const known2 = this.history.get(path) ?? [];
      const first = known2[0]?.[0] ?? Number.POSITIVE_INFINITY;
      const older = samples.filter(([time, value]) => Number.isFinite(time) && Number.isFinite(value) && time < first);
      const merged = [...older, ...known2];
      const newest = merged[merged.length - 1]?.[0] ?? 0;
      while (merged.length > 1 && (merged[1][0] < newest - spanMs || merged.length > MAX_SAMPLES)) merged.shift();
      this.history.set(path, merged);
    }
    /** A trend now shows `spanMs` of a path (§5.9 item 1): its samples are kept that long. */
    widen(path, spanMs) {
      this.trendSpansMs[path] = Math.max(this.trendSpansMs[path] ?? 0, spanMs);
    }
    /** Keeps a new sample of a trend path, inside its span plus the newest older sample, so the
     * trend can find the value at its left edge. Over a long span, one sample a second is kept. */
    sample(path, wire, atMs) {
      const spanMs = this.trendSpansMs[path];
      if (spanMs === void 0 || wire === null) return;
      const value = typeof wire.value === "boolean" ? Number(wire.value) : wire.value;
      if (typeof value !== "number" || !Number.isFinite(value)) return;
      const samples = this.history.get(path) ?? [];
      const last = samples[samples.length - 1];
      if (spanMs > LONG_SPAN_MS && last && Math.floor(last[0] / 1e3) === Math.floor(atMs / 1e3)) samples.pop();
      samples.push([atMs, value]);
      while (samples.length > 1 && (samples[1][0] < atMs - spanMs || samples.length > MAX_SAMPLES)) samples.shift();
      this.history.set(path, samples);
    }
  };
  var HISTORY_PATHS = 16;
  async function loadHistory(fetchJson, spansMs, store) {
    const paths = Object.keys(spansMs);
    if (paths.length === 0) return;
    const span = Math.min(86400, Math.max(1, Math.ceil(Math.max(...Object.values(spansMs)) / 1e3)));
    for (let start2 = 0; start2 < paths.length; start2 += HISTORY_PATHS) {
      const chunk = paths.slice(start2, start2 + HISTORY_PATHS);
      const query = chunk.map((path) => `path=${encodeURIComponent(path)}`).join("&");
      try {
        const answer = await fetchJson(`/api/hmi/history?${query}&span=${span}`);
        const body = answer.body;
        if (answer.status !== 200 || !body?.samples) continue;
        for (const path of chunk) store.seed(path, body.samples[path] ?? []);
      } catch {
      }
    }
  }

  // src/hmiBuilder/drawing/alarms.ts
  var NAMES = { high: "High", medium: "Medium", low: "Low" };
  function alarmPriority(value) {
    return value === "high" || value === "medium" ? value : "low";
  }
  function priorityName(priority) {
    return say(NAMES[priority]);
  }
  var STATES = {
    ActiveUnacknowledged: "Unacknowledged",
    ActiveAcknowledged: "Acknowledged",
    ClearedUnacknowledged: "Cleared, unacknowledged",
    Normal: "Normal"
  };
  function stateName(state) {
    const word = STATES[state];
    return word ? say(word) : state;
  }
  var HIDDEN = { shelved: "Shelved", suppressed: "Suppressed", out_of_service: "Out of service" };
  function rowStateName(row, tab) {
    const hidden = HIDDEN[tab];
    return hidden ? say(row.active ? "{state}, condition active" : "{state}, condition clear", { state: say(hidden) }) : stateName(row.state);
  }

  // src/hmiBuilder/drawing/valueFormats.ts
  function formatPath(path) {
    return path.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
  }
  function withValueFormat(object, frame, property = "value") {
    const binding = object.bind[property];
    if (!binding) return object;
    const format = frame.formats?.[formatPath(binding.path)];
    const equipment = frame.values?.[binding.path]?.presentation;
    const unit = object.unit ?? binding.unit ?? equipment?.unit ?? format?.unit;
    const decimals = object.decimals ?? format?.decimals;
    const range = object.type === "bar" || object.type === "gauge" ? equipment?.range : void 0;
    const min = object.min ?? range?.[0], max = object.max ?? range?.[1];
    if (unit === object.unit && decimals === object.decimals && min === object.min && max === object.max) return object;
    return {
      ...object,
      ...unit !== void 0 ? { unit } : {},
      ...decimals !== void 0 ? { decimals } : {},
      ...min !== void 0 ? { min } : {},
      ...max !== void 0 ? { max } : {}
    };
  }

  // src/hmiBuilder/drawing/values.ts
  var UNKNOWN = { value: null, state: "unknown" };
  function withQuality(value, quality) {
    value = value ?? UNKNOWN;
    if (!quality || quality.state === "unknown" || quality.staleMs !== void 0) return { ...value, state: "unknown" };
    const status = String(quality.value).toLowerCase();
    if (status === "true" || status === "good") return value;
    if (status === "uncertain") return { ...value, state: "estimated" };
    return { ...value, state: "unknown" };
  }

  // src/hmiBuilder/drawing/components/trend.ts
  var LIMITS = ["high-limit", "low-limit"];
  var NOT_PENS = [...LIMITS, "visible"];

  // src/hmiBuilder/display/trendActions.ts
  var SHORTEST_MS = 1e4;
  var LONGEST_MS = 864e5;
  var TrendActions = class {
    constructor(model, store, fetchJson, surface, now, views = {}, download, expand) {
      this.model = model;
      this.store = store;
      this.fetchJson = fetchJson;
      this.surface = surface;
      this.now = now;
      this.views = views;
      this.download = download;
      this.expand = expand;
    }
    /** A press on a trend: on the plot it starts a box, on the time row a pan; whether it was one. */
    press(objectId, at, target) {
      const drawn = this.drawn(objectId);
      if (!drawn) return false;
      if (target?.closest('[data-part="trend-time"]')) {
        this.gesture = { kind: "pan", objectId, drawn, fromX: at.x };
        return true;
      }
      const { plot } = drawn;
      if (at.x < plot.x || at.x > plot.x + plot.width || at.y < plot.y || at.y > plot.y + plot.height) return false;
      const time = timeAt(drawn, at.x);
      this.gesture = { kind: "box", objectId, drawn };
      this.views[objectId] = { ...this.views[objectId], box: { fromMs: time, toMs: time } };
      return true;
    }
    /** The pointer moved; whether a gesture follows it. */
    drag(at) {
      const gesture = this.gesture;
      if (!gesture) return false;
      const view = this.views[gesture.objectId] ?? {};
      if (gesture.kind === "box" && view.box) {
        this.views[gesture.objectId] = { ...view, box: { ...view.box, toMs: timeAt(gesture.drawn, at.x) } };
      } else if (gesture.kind === "pan") {
        const { drawn } = gesture;
        const span = drawn.end - drawn.start;
        this.window(gesture.objectId, span, drawn.end - (at.x - gesture.fromX) / drawn.plot.width * span);
      }
      return true;
    }
    /** The pointer was released: a box wide enough zooms into its period. */
    release(at) {
      const gesture = this.gesture;
      this.gesture = void 0;
      if (!gesture) return false;
      if (gesture.kind === "box") {
        const { box, ...view } = this.views[gesture.objectId] ?? {};
        this.views[gesture.objectId] = view;
        const to = timeAt(gesture.drawn, at.x);
        const from = box?.fromMs ?? to;
        if (Math.abs(to - from) * 100 >= gesture.drawn.end - gesture.drawn.start)
          this.window(gesture.objectId, Math.abs(to - from), Math.max(from, to));
        else this.views[gesture.objectId] = { ...this.views[gesture.objectId], cursorMs: to };
      }
      return true;
    }
    /** The pointer over a trend's plot puts the cursor there; off the plot the cursor goes. Whether
     * a cursor moved. */
    hover(objectId, at) {
      if (this.gesture) return false;
      let changed = false;
      for (const [id, view2] of Object.entries(this.views)) {
        if (id !== objectId && view2.cursorMs !== void 0) {
          const { cursorMs: _gone, ...rest } = view2;
          this.views[id] = rest;
          changed = true;
        }
      }
      const drawn = objectId ? this.drawn(objectId) : void 0;
      if (!objectId || !drawn) return changed;
      const { plot } = drawn;
      const inside = at.x >= plot.x && at.x <= plot.x + plot.width && at.y >= plot.y && at.y <= plot.y + plot.height;
      const view = this.views[objectId] ?? {};
      if (inside) this.views[objectId] = { ...view, cursorMs: timeAt(drawn, at.x) };
      else if (view.cursorMs !== void 0) {
        const { cursorMs: _gone, ...rest } = view;
        this.views[objectId] = rest;
      } else return changed;
      return true;
    }
    /** The wheel over a trend's plot zooms its time axis at the pointer; whether it did. */
    wheel(objectId, at, deltaY) {
      const drawn = this.drawn(objectId);
      if (!drawn || deltaY === 0) return false;
      const { plot } = drawn;
      if (at.x < plot.x || at.x > plot.x + plot.width || at.y < plot.y || at.y > plot.y + plot.height) return false;
      const span = drawn.end - drawn.start;
      const time = timeAt(drawn, at.x);
      const share = (time - drawn.start) / span;
      const next = Math.min(LONGEST_MS, Math.max(SHORTEST_MS, Math.round(span * (deltaY < 0 ? 0.8 : 1.25))));
      this.window(objectId, next, time - share * next + next);
      return true;
    }
    /** The visible window as CSV: the time of each sample in it, ISO 8601 in UTC, and each shown
     * pen's value then, held from its last sample; empty before its first. */
    export(objectId) {
      const object = this.model().object.find((candidate) => candidate.id === objectId);
      const drawn = this.drawn(objectId);
      if (!object || !drawn || !this.download) return;
      const hidden = this.views[objectId]?.hidden ?? [];
      const history = this.store.frame(this.now()).history ?? {};
      const pens = Object.entries(object.bind).filter(([name]) => !NOT_PENS.includes(name) && !hidden.includes(name)).sort(([first], [second]) => first < second ? -1 : first > second ? 1 : 0).map(([name, binding]) => ({ name, samples: history[binding.path] ?? [] }));
      const times = [...new Set(pens.flatMap((pen) => pen.samples.map(([time]) => time)))].filter((time) => time >= drawn.start && time <= drawn.end).sort((first, second) => first - second);
      const rows = times.map((time) => [
        new Date(time).toISOString(),
        ...pens.map((pen) => {
          const held = pen.samples.filter(([at]) => at <= time).pop();
          return held === void 0 ? "" : String(held[1]);
        })
      ]);
      const text2 = [["time", ...pens.map((pen) => pen.name)], ...rows].map((row) => row.join(",")).join("\n") + "\n";
      const stamp = new Date(this.now()).toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
      this.download(`${objectId}-${stamp}.csv`, text2);
    }
    /** Shows `spanMs` ending at `endMs`: live when it reaches now, else paused there. */
    window(objectId, spanMs, endMs) {
      const { endMs: _paused, ...view } = this.views[objectId] ?? {};
      const end = Math.round(endMs);
      this.views[objectId] = end >= this.now() ? { ...view, spanMs } : { ...view, spanMs, endMs: end };
      if (spanMs > (view.spanMs ?? 0)) void this.fill(objectId, spanMs);
    }
    /** The trend's plot on the page and its window, as last drawn. */
    drawn(objectId) {
      const object = this.model().object.find((candidate) => candidate.id === objectId && candidate.type === "trend");
      const group = this.surface.querySelector(`[data-object=${JSON.stringify(objectId)}]`);
      const trend = group?.matches(".hmi-trend") ? group : group?.querySelector(".hmi-trend");
      const plot = trend?.querySelector('[data-part="plot"]');
      if (!object || !trend || !plot) return void 0;
      const number = (element2, name) => Number(element2.getAttribute(name));
      return {
        plot: {
          x: object.at[0] + number(plot, "x"),
          y: object.at[1] + number(plot, "y"),
          width: number(plot, "width") || 1,
          height: number(plot, "height") || 1
        },
        start: number(trend, "data-window-start"),
        end: number(trend, "data-window-end")
      };
    }
    /** A click on one of a trend's chips; whether it was one. `redraw` shows the change. */
    click(clicked, objectId, redraw) {
      if (!clicked || !objectId) return false;
      const range = clicked.closest("[data-trend-range]")?.getAttribute("data-trend-range");
      if (range) {
        const spanMs = Number(range) * 1e3;
        this.views[objectId] = { ...this.views[objectId], spanMs };
        void this.fill(objectId, spanMs).then(redraw);
        redraw();
        return true;
      }
      const pen = clicked.closest('[data-part="legend"][data-pen]')?.getAttribute("data-pen");
      if (pen) {
        const hidden = this.views[objectId]?.hidden ?? [];
        this.views[objectId] = {
          ...this.views[objectId],
          hidden: hidden.includes(pen) ? hidden.filter((name) => name !== pen) : [...hidden, pen]
        };
        redraw();
        return true;
      }
      if (clicked.closest('[data-trend-action="export"]')) {
        this.export(objectId);
        return true;
      }
      if (clicked.closest('[data-trend-action="expand"]')) {
        this.expand?.(objectId);
        return true;
      }
      if (clicked.closest('[data-trend-action="live"]')) {
        const { endMs: _paused, ...live } = this.views[objectId] ?? {};
        this.views[objectId] = live;
        redraw();
        return true;
      }
      return false;
    }
    /** The trend's pens keep `spanMs` of samples, filled from the runtime's history. */
    async fill(objectId, spanMs) {
      const object = this.model().object.find((candidate) => candidate.id === objectId);
      if (!object) return;
      const spans = {};
      for (const [name, binding] of Object.entries(object.bind)) {
        if (NOT_PENS.includes(name)) continue;
        this.store.widen(binding.path, spanMs);
        spans[binding.path] = spanMs;
      }
      await loadHistory(this.fetchJson, spans, this.store);
    }
  };
  function timeAt(drawn, x) {
    const share = Math.min(1, Math.max(0, (x - drawn.plot.x) / drawn.plot.width));
    return Math.round(drawn.start + share * (drawn.end - drawn.start));
  }
  var TREND_CSS = `
.hmi-trend-expanded { position: fixed; inset: 0; z-index: 25; padding: 16px; background: var(--hmi-page); }
.hmi-trend-expanded .hmi-trend-surface { width: 100%; height: 100%; }
.hmi-trend-expanded .hmi-trend-surface > svg { width: 100%; height: 100%; }
.hmi-trend-expanded .hmi-trend-close { position: absolute; top: 20px; right: 24px; z-index: 1; width: 40px; height: 40px; border-radius: 20px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 18px var(--hmi-font); cursor: pointer; }
`;

  // src/hmiBuilder/display/motionLayer.ts
  var SVG_NAMESPACE2 = "http://www.w3.org/2000/svg";
  var TURNING = ".hmi-spin";
  var MOTION_LAYER_CSS = `.hmi-surface { position: relative; }
[data-motion-layer] { position: absolute; left: 0; top: 0; width: 0; height: 0; overflow: visible; pointer-events: none; }
[data-motion-layer] > svg { position: absolute; overflow: visible; will-change: transform; }
[data-moved] { visibility: hidden; animation: none; }`;
  function measured(part) {
    const graphic = part;
    if (typeof graphic.getBBox !== "function" || typeof graphic.getCTM !== "function") return void 0;
    const box = graphic.getBBox();
    const root = graphic.closest("svg.hmi-page");
    const screen = graphic.getScreenCTM?.();
    const rootScreen = root?.getScreenCTM?.();
    const rootViewport = root?.getCTM?.();
    const matrix = screen && rootScreen && rootViewport ? rootViewport.multiply(rootScreen.inverse()).multiply(screen) : graphic.getCTM();
    if (!matrix || !(box.width > 0) || !(box.height > 0)) return void 0;
    return {
      box: { x: box.x, y: box.y, width: box.width, height: box.height },
      matrix: { a: matrix.a, b: matrix.b, c: matrix.c, d: matrix.d, e: matrix.e, f: matrix.f }
    };
  }
  var MotionLayer = class {
    constructor(surface, measure = measured) {
      this.surface = surface;
      this.measure = measure;
      this.lifted = /* @__PURE__ */ new Map();
    }
    /** After each drawing of the page: an element for each turning part, the stopped ones given back. */
    sync() {
      const page = this.surface.querySelector("svg.hmi-page");
      const turning = new Set(page ? Array.from(page.querySelectorAll(TURNING)) : []);
      for (const [part, lifted] of this.lifted) {
        if (turning.has(part) && part.isConnected) continue;
        this.giveBack(part, lifted);
      }
      for (const part of turning) {
        const place = placeOf(part);
        const known2 = this.lifted.get(part);
        if (known2 && known2.place === place) {
          if (known2.drawing !== part.innerHTML) {
            copyDrawing(known2.element, part);
            known2.drawing = part.innerHTML;
          }
          continue;
        }
        const placement = this.measure(part);
        if (!placement) {
          if (known2) this.giveBack(part, known2);
          continue;
        }
        const element2 = known2?.element ?? this.created(part);
        copyDrawing(element2, part);
        position(element2, part, placement);
        this.lifted.set(part, { element: element2, drawing: part.innerHTML, place });
        part.setAttribute("data-moved", "");
      }
    }
    giveBack(part, lifted) {
      lifted.element.remove();
      part.removeAttribute("data-moved");
      this.lifted.delete(part);
    }
    created(part) {
      const document2 = this.surface.ownerDocument;
      if (!this.layer?.isConnected) {
        this.layer = document2.createElement("div");
        this.layer.setAttribute("data-motion-layer", "");
        this.surface.append(this.layer);
      }
      const element2 = document2.createElementNS(SVG_NAMESPACE2, "svg");
      element2.setAttribute("class", "hmi-spin");
      element2.setAttribute("data-motion", part.closest("[data-object]")?.getAttribute("data-object") ?? "");
      element2.append(document2.createElementNS(SVG_NAMESPACE2, "g"));
      this.layer.append(element2);
      return element2;
    }
  };
  function placeOf(part) {
    const transforms = [];
    for (let node = part.parentElement; node; node = node.parentElement) {
      transforms.push(node.getAttribute("transform") ?? "");
      if (node.tagName.toLowerCase() === "svg") {
        transforms.push(
          ...["x", "y", "width", "height", "viewBox", "preserveAspectRatio"].map(
            (name) => node.getAttribute(name) ?? ""
          )
        );
      }
      if (node.matches("svg.hmi-page")) break;
    }
    return transforms.join("|");
  }
  function copyDrawing(element2, part) {
    const turned = element2.firstElementChild;
    if (!turned) return;
    turned.replaceChildren(...Array.from(part.childNodes, (node) => node.cloneNode(true)));
  }
  function position(element2, part, { box, matrix: m }) {
    const centreX = box.x + box.width / 2;
    const centreY = box.y + box.height / 2;
    const x = m.a * centreX + m.c * centreY + m.e;
    const y = m.b * centreX + m.d * centreY + m.f;
    const scale = Math.hypot(m.a, m.b);
    const angle = Math.atan2(m.b, m.a) * 180 / Math.PI;
    const width = box.width * scale;
    const height = box.height * scale;
    element2.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
    element2.style.left = `${round(x - width / 2)}px`;
    element2.style.top = `${round(y - height / 2)}px`;
    element2.style.width = `${round(width)}px`;
    element2.style.height = `${round(height)}px`;
    const delay = /animation-delay:\s*([^;]+)/.exec(part.getAttribute("style") ?? "")?.[1]?.trim();
    if (delay) element2.style.animationDelay = delay;
    const turned = element2.firstElementChild;
    if (!turned) return;
    if (Math.abs(angle) > 0.01) turned.setAttribute("transform", `rotate(${round(angle)} ${centreX} ${centreY})`);
    else turned.removeAttribute("transform");
  }
  function round(value) {
    return Math.round(value * 100) / 100;
  }

  // src/hmiBuilder/display/equipmentNavigation.ts
  function match(object, equipment) {
    if (object.instance === equipment || object.click?.instance === equipment) return 2;
    if (Object.values(object.bind).some((binding) => binding.path.startsWith(`${equipment}.`)))
      return object.click?.faceplate ? 2 : 1;
    return 0;
  }
  async function equipmentDestination(host2, equipment) {
    if (!equipment) return void 0;
    const name = decodeURIComponent(host2.location.pathname.split("/").pop() ?? "");
    const first = await host2.fetchJson(`/api/hmi/page?name=${encodeURIComponent(name)}`);
    if (first.status !== 200) throw new Error("page lookup unavailable");
    const initial = first.body;
    const names = [
      ...new Set(
        [initial.page?.name, ...(initial.navigation ?? []).map((page) => page.name), ...initial.pages].filter(
          (value) => Boolean(value)
        )
      )
    ];
    for (const pageName of names) {
      const answer = pageName === initial.page?.name ? first : await host2.fetchJson(`/api/hmi/page?name=${encodeURIComponent(pageName)}`);
      if (answer.status !== 200) continue;
      const data = answer.body;
      if (data.bundle !== initial.bundle) throw new Error("the HMI changed during lookup");
      const objects = data.page?.model.object ?? [];
      const object = objects.find((object2) => match(object2, equipment) === 2) ?? objects.find((object2) => match(object2, equipment) === 1);
      if (object) return `/hmi/${encodeURIComponent(pageName)}?focus=${encodeURIComponent(object.id)}`;
    }
    return void 0;
  }
  var EquipmentFocus = class {
    constructor(host2, redraw) {
      this.host = host2;
      this.scrolled = false;
      this.id = new URLSearchParams(host2.location.search ?? "").get("focus");
      this.expires = host2.now() + 5e3;
      if (this.id) host2.later(5e3, redraw);
    }
    apply(surface) {
      if (!this.id) return;
      surface.querySelectorAll("[data-focused]").forEach((node) => node.removeAttribute("data-focused"));
      if (this.host.now() >= this.expires) return;
      const target = Array.from(surface.querySelectorAll("[data-object]")).find(
        (node) => node.getAttribute("data-object") === this.id
      );
      if (!target) return;
      target.setAttribute("data-focused", "true");
      if (!this.scrolled) {
        target.scrollIntoView?.({ block: "nearest", inline: "nearest" });
        this.scrolled = true;
      }
    }
  };
  var EQUIPMENT_FOCUS_CSS = `.hmi-surface [data-focused="true"] { outline: 2px dashed var(--hmi-accent); outline-offset: 6px; }`;

  // src/hmiBuilder/display/commandReason.ts
  var NOT_ACCEPTED = "not accepted";
  function commandReason(outcome, reason) {
    const said = reason.trim();
    if (outcome !== "refused") return said;
    if (said === "" || said.toLowerCase() === NOT_ACCEPTED) return say("Not accepted");
    return say("Not accepted: {reason}", { reason: said });
  }

  // src/hmiBuilder/drawing/components/controls.ts
  function roleWord(role) {
    return role === "engineer" ? say("Engineer") : role === "operator" ? say("Operator") : say("Locked");
  }

  // src/hmiBuilder/drawing/components/slider.ts
  var THUMB = 11;
  var NUMBER_ROOM = 64;
  var LABEL_ROOM = 22;
  function sliderLimits(object) {
    return [object.min ?? 0, object.max ?? 100];
  }
  function sliderTrack(object) {
    const [width, height] = object.size;
    const label = object.label ? LABEL_ROOM : 0;
    if (object.variant === "vertical") return [width / 2, height - THUMB - 4, width / 2, label + NUMBER_ROOM / 2 + THUMB];
    const y = object.label ? Math.max(height / 2, label + (height - label) / 2) : height / 2;
    return [THUMB + 4, y, width - NUMBER_ROOM - THUMB, y];
  }
  function sliderValueAt(object, x, y) {
    const [x1, y1, x2, y2] = sliderTrack(object);
    const [min, max] = sliderLimits(object);
    const local = { x: x - object.at[0], y: y - object.at[1] };
    const length = Math.hypot(x2 - x1, y2 - y1);
    const along = length === 0 ? 0 : ((local.x - x1) * (x2 - x1) + (local.y - y1) * (y2 - y1)) / (length * length);
    const share = Math.min(1, Math.max(0, along));
    const factor = 10 ** (object.decimals ?? 0);
    return Math.round((min + share * (max - min)) * factor) / factor;
  }

  // src/hmiBuilder/display/gestures.ts
  var NOT_KNOWN = () => say("The value of this control is not known, so it sends nothing.");
  function known(object, property, context) {
    const live = context.live(object, property);
    if (!live || live.state === "unknown" || live.staleMs !== void 0 || live.value === null) {
      context.notice(NOT_KNOWN());
      return void 0;
    }
    return live;
  }
  function toggleSwitch(object, context) {
    const live = known(object, "value", context);
    if (live) context.write(object, live.value !== true);
  }
  function openSelector(object, context) {
    const live = known(object, "value", context);
    if (!live) return;
    const options = object.options ?? [];
    const current2 = typeof live.value === "number" ? options[live.value] : String(live.value);
    const dialog = context.dialog("choices");
    const document2 = dialog.ownerDocument;
    const heading = document2.createElement("h2");
    heading.textContent = object.label !== void 0 ? translate(object.label) : say("Choose");
    const list = document2.createElement("div");
    list.className = "hmi-choices";
    for (const option of options) {
      const choice = document2.createElement("button");
      choice.type = "button";
      choice.textContent = option;
      choice.setAttribute("data-option", option);
      const chosen = option.toLowerCase() === current2?.toLowerCase();
      choice.setAttribute("aria-pressed", String(chosen));
      choice.addEventListener("click", () => {
        context.closeDialog();
        if (!chosen) context.write(object, option);
      });
      list.append(choice);
    }
    const cancel = document2.createElement("button");
    cancel.type = "button";
    cancel.textContent = say("Cancel");
    cancel.setAttribute("data-action", "cancel");
    cancel.addEventListener("click", () => context.closeDialog());
    const actions = document2.createElement("div");
    actions.className = "hmi-actions";
    actions.append(cancel);
    dialog.append(heading, list, actions);
  }
  function chooseMode(object, mode, context) {
    if (mode === void 0) return;
    const live = known(object, "mode", context);
    if (live && String(live.value).toLowerCase() !== mode.toLowerCase()) context.write(object, mode);
  }
  var SliderDrag = class _SliderDrag {
    constructor(object, context, at) {
      this.object = object;
      this.context = context;
      this.value = sliderValueAt(object, at.x, at.y);
      context.draft(object, this.value);
    }
    /** A drag that starts at a point of the page, or none while the value is not known. */
    static start(object, at, context) {
      return known(object, "value", context) ? new _SliderDrag(object, context, at) : void 0;
    }
    move(at) {
      this.value = sliderValueAt(this.object, at.x, at.y);
      this.context.draft(this.object, this.value);
    }
    end(at) {
      this.move(at);
      this.context.write(this.object, this.value);
    }
  };

  // src/hmiBuilder/display/overlay.ts
  var NOTICE_MS = 6e3;
  var OPERATE_CSS = `
.hmi-keypad { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin: 14px 0 4px; }
.hmi-dialog .hmi-keypad button { height: 56px; padding: 0; border-radius: 12px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 24px var(--hmi-font); cursor: pointer; }
.hmi-dialog .hmi-keypad button[data-key="back"] { grid-column: span 3; font-size: 22px; }
.hmi-dialog .hmi-keypad ~ .hmi-actions button { min-height: 52px; padding: 12px 28px; font-size: 17px; }
.hmi-session { position: fixed; top: 12px; right: 12px; z-index: 10; display: flex; align-items: center; gap: 10px; padding: 5px 6px 5px 14px; border-radius: 999px; background: var(--hmi-raised); border: 1px solid var(--hmi-border); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35); font: 500 13px var(--hmi-font); color: var(--hmi-text); }
.hmi-session .hmi-role { padding: 2px 9px; border-radius: 999px; background: color-mix(in srgb, var(--hmi-accent) 18%, transparent); color: var(--hmi-accent); font-size: 12px; font-weight: 600; }
.hmi-session button, .hmi-dialog button { box-sizing: border-box; min-width: 44px; min-height: 44px; font: 600 14px var(--hmi-font); border-radius: 999px; padding: 6px 14px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-surface); color: var(--hmi-text); }
.hmi-dialog button[type="submit"], .hmi-dialog [data-action="confirm"] { background: var(--hmi-accent); border-color: var(--hmi-accent); color: var(--hmi-page); }
.hmi-backdrop { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(3px); }
.hmi-dialog { min-width: 320px; max-width: 440px; display: grid; gap: 14px; padding: 24px; border-radius: 16px; background: var(--hmi-surface); border: 1px solid var(--hmi-border); box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5); font: 14px/1.45 var(--hmi-font); color: var(--hmi-text); }
.hmi-dialog h2 { margin: 0; font: 650 18px var(--hmi-font); }
.hmi-dialog label { display: grid; gap: 6px; color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog input { box-sizing: border-box; min-height: 44px; font: 500 16px var(--hmi-font); padding: 10px 12px; border-radius: 10px; border: 1px solid var(--hmi-border); background: var(--hmi-page); color: var(--hmi-text); }
.hmi-dialog .hmi-actions { display: flex; justify-content: flex-end; gap: 10px; }
.hmi-dialog .hmi-hint { margin: 0; color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog .hmi-error { margin: 0; color: var(--hmi-alarm); font-size: 13px; }
.hmi-notice { position: fixed; left: 50%; bottom: 24px; z-index: 30; transform: translateX(-50%); max-width: min(560px, 90vw); padding: 10px 16px; border-radius: 12px; background: var(--hmi-raised); border: 1px solid var(--hmi-alarm); box-shadow: 0 12px 34px rgba(0, 0, 0, 0.45); font: 500 14px var(--hmi-font); color: var(--hmi-text); }
.hmi-reload { display: flex; align-items: center; gap: 14px; top: 72px; bottom: auto; border-color: var(--hmi-accent); }
.hmi-choices { display: grid; gap: 8px; }
.hmi-choices button { text-align: left; font: 600 15px var(--hmi-font); padding: 12px 16px; min-height: 44px; border-radius: 12px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-page); color: var(--hmi-text); }
.hmi-choices button[aria-pressed="true"] { border-color: var(--hmi-accent); color: var(--hmi-accent); box-shadow: inset 3px 0 0 var(--hmi-accent); }
.hmi-dialog .hmi-alarm-title { display: flex; align-items: center; gap: 10px; }
.hmi-dialog .hmi-alarm-mark { display: inline-flex; flex: none; }
.hmi-dialog .hmi-facts { display: grid; grid-template-columns: auto 1fr; gap: 6px 18px; margin: 0; }
.hmi-dialog .hmi-facts dt { color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog .hmi-facts dd { margin: 0; font-weight: 600; overflow-wrap: anywhere; }
.hmi-dialog .hmi-alarm-actions, .hmi-dialog .hmi-shelve { justify-content: flex-start; flex-wrap: wrap; }
.hmi-dialog .hmi-alarm-actions button, .hmi-dialog .hmi-shelve button { min-height: 44px; padding: 8px 16px; }
.hmi-dialog [data-action="acknowledge"] { background: var(--hmi-accent); border-color: var(--hmi-accent); color: var(--hmi-page); }
.hmi-reload button { box-sizing: border-box; min-width: 44px; min-height: 44px; font: 600 14px var(--hmi-font); border-radius: 999px; padding: 6px 14px; cursor: pointer; border: 1px solid var(--hmi-accent); background: var(--hmi-accent); color: var(--hmi-page); }
`;
  var Overlay = class {
    constructor(container, host2) {
      this.host = host2;
      /** The last press on the page or a popup was a touch: number entries open the on-screen keypad
       * (specification 39, §18 phase 5 item 6). */
      this.touched = false;
      this.noticeTimer = 0;
      this.layer = container.ownerDocument.createElement("div");
      this.layer.className = "hmi-operate";
      container.append(this.layer);
    }
    /** A short notice at the bottom that goes after a few seconds. */
    notice(text2) {
      this.layer.querySelector('[data-part="notice"]')?.remove();
      const note = element(this.layer, "div", text2, "hmi-notice");
      note.setAttribute("data-part", "notice");
      note.setAttribute("role", "status");
      this.layer.append(note);
      const mine = this.noticeTimer += 1;
      this.host.later(NOTICE_MS, () => {
        if (mine === this.noticeTimer) note.remove();
      });
    }
    /** Takes away the notice now: a later command was accepted, so an earlier refusal is past. */
    clearNotice() {
      this.noticeTimer += 1;
      this.layer.querySelector('[data-part="notice"]')?.remove();
    }
    /** A newer bundle is active: the page tells its user to reload; its commands would be refused
     * as out of date (§16.2.6). */
    showReload() {
      if (this.layer.querySelector('[data-part="reload"]')) return;
      const note = element(this.layer, "div", "", "hmi-notice hmi-reload");
      note.setAttribute("data-part", "reload");
      note.setAttribute("role", "alert");
      note.append(element(note, "span", say("This page has changed. Reload to see the new version.")));
      const again = button(note, say("Reload"), "reload");
      again.addEventListener("click", () => this.host.reload());
      note.append(again);
      this.layer.append(note);
    }
    /** A new dialog in place of any open one. */
    dialog(tag, part) {
      this.closeDialog();
      const document2 = this.layer.ownerDocument;
      const backdrop = document2.createElement("div");
      backdrop.className = "hmi-backdrop";
      backdrop.setAttribute("data-part", "backdrop");
      const dialog = document2.createElement(tag);
      dialog.className = "hmi-dialog";
      dialog.setAttribute("data-part", part);
      dialog.setAttribute("role", "dialog");
      backdrop.append(dialog);
      this.layer.append(backdrop);
      return dialog;
    }
    closeDialog() {
      this.layer.querySelector('[data-part="backdrop"]')?.remove();
    }
  };
  function element(parent, tag, text2, className) {
    const created = parent.ownerDocument.createElement(tag);
    if (text2) created.textContent = text2;
    if (className) created.className = className;
    return created;
  }
  function button(parent, text2, action) {
    const created = parent.ownerDocument.createElement("button");
    created.type = "button";
    created.textContent = text2;
    if (action) created.setAttribute("data-action", action);
    return created;
  }
  function labelledInput(form, text2, name, autocomplete) {
    const label = element(form, "label", text2);
    const input = form.ownerDocument.createElement("input");
    input.name = name;
    input.setAttribute("autocomplete", autocomplete);
    label.append(input);
    form.append(label);
    return input;
  }

  // src/hmiBuilder/display/commands.ts
  var BUTTONS = ["button", "pulse-button"];
  var ENTRIES = ["setpoint", "numeric-input"];
  var CHOOSERS = ["switch", "selector", "mode-selector"];
  var OUTCOME_FADE_MS = 4e3;
  var CommandSurface = class {
    constructor(host2, surface, bundle, session, overlay, redraw, liveAt) {
      this.host = host2;
      this.surface = surface;
      this.bundle = bundle;
      this.session = session;
      this.overlay = overlay;
      this.redraw = redraw;
      this.liveAt = liveAt;
      /** The latest outcome on each control, by object id. */
      this.outcomes = /* @__PURE__ */ new Map();
      /** The control each command came from, by command id. */
      this.sentFrom = /* @__PURE__ */ new Map();
      /** Values being entered on controls, by object id, until they are sent (§5.8). */
      this.entered = /* @__PURE__ */ new Map();
      /** What the gestures of §5.8 use. */
      this.gestures = {
        live: (object, property) => {
          const binding = object.bind[property];
          const value = this.liveAt(binding?.path ?? "");
          return binding?.quality ? withQuality(value, this.liveAt(binding.quality)) : value;
        },
        write: (object, value) => this.asked(
          object,
          () => void this.send(object, "write", value),
          () => this.dropDraft(object.id)
        ),
        notice: (text2) => this.overlay.notice(text2),
        dialog: (part) => this.overlay.dialog("div", part),
        closeDialog: () => this.overlay.closeDialog(),
        draft: (object, value) => {
          if (value === void 0) this.entered.delete(object.id);
          else this.entered.set(object.id, value);
          this.redraw();
        }
      };
    }
    /** The latest outcome on each control. */
    states() {
      return Object.fromEntries(this.outcomes);
    }
    /** The values being entered on controls. */
    drafts() {
      return Object.fromEntries(this.entered);
    }
    /** The controls the signed-in role may not use: the role that may, or "none" (§5.9 item 5).
     * Without a session every control is read-only instead. */
    locked() {
      const role = this.session.role;
      if (this.session.readOnly || role === void 0) return {};
      const locked = {};
      for (const [id, needed] of Object.entries(this.surface.roles ?? {})) {
        if (needed === null) locked[id] = "none";
        else if (rank(role) < rank(needed)) locked[id] = needed;
      }
      return locked;
    }
    /** A later outcome from the stream; whether it was one of this surface's commands. */
    receive(command) {
      const from = this.sentFrom.get(command.id);
      if (!from) return false;
      this.setOutcome(from, { outcome: command.outcome, reason: command.reason });
      return true;
    }
    /** A click on a drawn object; `option` is the mode or option clicked inside it. */
    click(objectId, option) {
      const object = this.usable(objectId, [...BUTTONS, ...ENTRIES, ...CHOOSERS]);
      if (!object) return;
      if (BUTTONS.includes(object.type)) {
        const action = object.type === "pulse-button" ? "pulse" : object.action ?? "set";
        this.asked(object, () => void this.send(object, action));
      } else if (object.type === "switch") {
        toggleSwitch(object, this.gestures);
      } else if (object.type === "selector") {
        openSelector(object, this.gestures);
      } else if (object.type === "mode-selector") {
        chooseMode(object, option, this.gestures);
      } else {
        this.showEntry(object);
      }
    }
    /** A press on a drawn object at a point of the page; whether it started dragging a slider. */
    press(objectId, at) {
      const object = this.usable(objectId, ["slider"]);
      this.dragging = object ? SliderDrag.start(object, at, this.gestures) : void 0;
      return this.dragging !== void 0;
    }
    /** The pointer moved while a slider is dragged. */
    drag(at) {
      this.dragging?.move(at);
    }
    /** The pointer was released: a dragged slider writes its value. */
    release(at) {
      const dragging = this.dragging;
      this.dragging = void 0;
      dragging?.end(at);
    }
    /** The control with this id when it is one of `types` and may be used; without a session it
     * asks to log in instead. */
    usable(objectId, types) {
      const object = this.surface.model.object.find((candidate) => candidate.id === objectId);
      if (!object || !types.includes(object.type)) return void 0;
      if (this.session.readOnly) {
        this.session.showLogin(say("Log in to operate."));
        return void 0;
      }
      const locked = this.locked()[objectId];
      if (locked) {
        this.overlay.notice(
          locked === "none" ? say("Locked") : say("This needs the {role} role.", { role: roleWord(locked) })
        );
        return void 0;
      }
      return object;
    }
    /** A control's latest outcome; an applied or confirmed one fades after a few seconds. A refusal
     * reads "Not accepted: <reason>" beside the control (§21.2 item 3). */
    setOutcome(objectId, state) {
      if (state.reason) state = { ...state, reason: commandReason(state.outcome, state.reason) };
      this.outcomes.set(objectId, state);
      if (state.outcome !== "applied" && state.outcome !== "confirmed") return;
      this.host.later(OUTCOME_FADE_MS, () => {
        if (this.outcomes.get(objectId) !== state) return;
        this.outcomes.delete(objectId);
        this.redraw();
      });
    }
    /** Drops the refusal marks of the controls other than `except`: a refusal answered an earlier
     * click, and once a later command is accepted it no longer describes the equipment. Unknown and
     * timed-out outcomes stay, as those commands may still have acted. */
    dropRefusals(except) {
      for (const [id, state] of this.outcomes) {
        if (id !== except && state.outcome === "refused") this.outcomes.delete(id);
      }
    }
    dropDraft(objectId) {
      if (this.entered.delete(objectId)) this.redraw();
    }
    /** Runs `then` straight away, or after the control's question is answered yes; `cancelled`
     * when it is answered no. */
    asked(object, then, cancelled = () => void 0) {
      if (!object.confirm) {
        then();
        return;
      }
      const dialog = this.overlay.dialog("div", "confirm");
      dialog.append(element(dialog, "h2", object.confirm));
      const actions = element(dialog, "div", "", "hmi-actions");
      const cancel = button(dialog, say("Cancel"), "cancel");
      const yes = button(dialog, say("Send"), "confirm");
      cancel.addEventListener("click", () => {
        this.overlay.closeDialog();
        cancelled();
      });
      yes.addEventListener("click", () => {
        this.overlay.closeDialog();
        then();
      });
      actions.append(cancel, yes);
      dialog.append(actions);
    }
    async send(object, action, value) {
      const id = commandId();
      this.sentFrom.set(id, object.id);
      this.setOutcome(object.id, { outcome: "pending" });
      this.redraw();
      const body = {
        id,
        bundle: this.bundle,
        page: this.surface.name,
        object: object.id,
        action: object.command ? "invoke" : action
      };
      if (value !== void 0) body.value = value;
      let answer;
      try {
        answer = await this.host.postJson("/api/hmi/command", body);
      } catch {
        this.setOutcome(object.id, {
          outcome: "unknown",
          reason: say("The connection was lost before the outcome was known.")
        });
        this.overlay.notice(say("The connection was lost before the outcome was known."));
        this.entered.delete(object.id);
        this.redraw();
        return;
      }
      this.entered.delete(object.id);
      const reply = answer.body;
      if (answer.status === 200 && reply?.outcome) {
        this.setOutcome(object.id, { outcome: reply.outcome, reason: reply.reason });
        if (reply.outcome === "applied" || reply.outcome === "confirmed" || reply.outcome === "pending") {
          this.overlay.clearNotice();
          this.dropRefusals(object.id);
        }
        if ((reply.outcome === "refused" || reply.outcome === "failed") && reply.reason) {
          const owner = /^owner-login:([A-Za-z0-9_-]{1,128})$/.exec(reply.reason);
          if (owner)
            this.session.showLogin(
              say("Log in to {runtime} with a named account on that runtime. The command has not been sent.", {
                runtime: owner[1]
              }),
              owner[1]
            );
          else this.overlay.notice(commandReason(reply.outcome, reply.reason));
        }
      } else {
        const reason = reply?.error ?? say("The runtime answered {status}.", { status: answer.status });
        this.setOutcome(object.id, { outcome: "refused", reason });
        this.overlay.notice(reason);
      }
      this.redraw();
    }
    /** Asks for a number within the control's limits, then sends it. */
    showEntry(object) {
      const path = object.bind.value?.path ?? "";
      const live = this.liveAt(path);
      const unit = withValueFormat(object, { formats: this.surface.formats, values: live ? { [path]: live } : {} }).unit;
      const form = this.overlay.dialog("form", "entry");
      form.append(element(form, "h2", object.label !== void 0 ? translate(object.label) : say("New value")));
      const field2 = element(form, "label", `${say("Value")}${unit ? ` (${unit})` : ""}`);
      const input = form.ownerDocument.createElement("input");
      input.name = "value";
      input.inputMode = "decimal";
      input.setAttribute("autocomplete", "off");
      const current2 = live?.value;
      input.value = typeof current2 === "number" ? String(current2) : "";
      field2.append(input);
      form.append(field2);
      const limit2 = (value) => value === void 0 ? say("any") : formatValue(value, {});
      const limits = object.min !== void 0 || object.max !== void 0 ? `${say("Between {min} and {max}", { min: limit2(object.min), max: limit2(object.max) })}${unit ? ` ${unit}` : ""}` : "";
      if (limits) form.append(element(form, "p", limits, "hmi-hint"));
      const error = element(form, "p", "", "hmi-error");
      form.append(error);
      if (this.overlay.touched) {
        input.inputMode = "none";
        const keypad = element(form, "div", "", "hmi-keypad");
        keypad.setAttribute("data-part", "keypad");
        const keys = [
          ["7", "7"],
          ["8", "8"],
          ["9", "9"],
          ["4", "4"],
          ["5", "5"],
          ["6", "6"],
          ["1", "1"],
          ["2", "2"],
          ["3", "3"],
          ["sign", "\xB1"],
          ["0", "0"],
          [".", "."],
          ["back", "\u232B"]
        ];
        for (const [key, label] of keys) {
          const pressed = button(form, label);
          pressed.setAttribute("data-key", key);
          pressed.addEventListener("click", () => {
            if (key === "back") input.value = input.value.slice(0, -1);
            else if (key === "sign") input.value = input.value.startsWith("-") ? input.value.slice(1) : `-${input.value}`;
            else if (key !== "." || !input.value.includes(".")) input.value += key;
            error.textContent = "";
          });
          keypad.append(pressed);
        }
        form.append(keypad);
      }
      const actions = element(form, "div", "", "hmi-actions");
      const cancel = button(form, say("Cancel"), "cancel");
      cancel.type = "button";
      cancel.addEventListener("click", () => this.overlay.closeDialog());
      const submit = button(form, say("Send"));
      submit.type = "submit";
      actions.append(cancel, submit);
      form.append(actions);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const value = Number(input.value.trim().replace(",", "."));
        if (input.value.trim() === "" || !Number.isFinite(value)) {
          error.textContent = say("Enter a number.");
          return;
        }
        if (object.min !== void 0 && value < object.min || object.max !== void 0 && value > object.max) {
          error.textContent = say("Enter a number between {min} and {max}.", {
            min: limit2(object.min),
            max: limit2(object.max)
          });
          return;
        }
        this.overlay.closeDialog();
        this.asked(object, () => void this.send(object, "write", value));
      });
      input.focus();
    }
  };
  function commandId() {
    const random = new Uint32Array(3);
    const crypto = globalThis.crypto;
    if (crypto?.getRandomValues) crypto.getRandomValues(random);
    else for (let index = 0; index < random.length; index += 1) random[index] = Math.floor(Math.random() * 2 ** 32);
    return `c-${Date.now().toString(36)}-${Array.from(random, (part) => part.toString(36)).join("")}`;
  }
  function rank(role) {
    return ["viewer", "operator", "engineer", "admin"].indexOf(role);
  }

  // src/hmiBuilder/display/alarmActions.ts
  function durationName(seconds) {
    return seconds < 3600 ? say("{minutes} min", { minutes: seconds / 60 }) : say("{hours} h", { hours: seconds / 3600 });
  }
  var ACTIONS = {
    acknowledge: "Acknowledge",
    shelve: "Shelve",
    unshelve: "Unshelve",
    reset: "Reset",
    comment: "Comment",
    "remove-from-service": "Take out of service",
    "return-to-service": "Return to service"
  };
  var OUTCOMES = {
    refused: "Not accepted",
    "timed-out": "Timed out",
    unknown: "Unknown"
  };
  function outcomeSaid(outcome, reason) {
    if (reason) return commandReason(outcome, reason);
    const named = OUTCOMES[outcome];
    return named ? say(named) : outcome;
  }
  var TABS = ["active", "shelved", "suppressed", "out_of_service"];
  var AlarmActions = class {
    constructor(host2, session, overlay, mark = () => "") {
      this.host = host2;
      this.session = session;
      this.overlay = overlay;
      this.mark = mark;
      /** What each action in flight was, by command id, for its later outcome. */
      this.sent = /* @__PURE__ */ new Map();
    }
    /**
     * A click in a drawn surface: a tab, a page, an action button or a row of the alarm list with
     * `objectId`. Whether it was one; `redraw` shows a changed tab or page.
     */
    click(clicked, objectId, view, redraw) {
      if (!clicked || !objectId) return false;
      const equipment = clicked.closest('[data-part="equipment-alarm"]')?.getAttribute("data-alarm");
      if (equipment) {
        const row = view.summary?.active.find((candidate) => candidate.path === equipment);
        if (row) this.open(row, "active", view.summary);
        return true;
      }
      const tab = clicked.closest("[data-alarm-tab]")?.getAttribute("data-alarm-tab");
      if (tab && TABS.includes(tab)) {
        view.views[objectId] = { ...view.views[objectId], tab, first: 0 };
        redraw();
        return true;
      }
      const chip2 = clicked.closest("[data-alarm-filter]");
      if (chip2) {
        const current2 = view.views[objectId] ?? {};
        const filter = { ...current2.filter };
        const kind = chip2.getAttribute("data-alarm-filter");
        if (kind === "priority") {
          const value = chip2.getAttribute("data-value") ?? "";
          const chosen = filter.priorities ?? [];
          filter.priorities = chosen.includes(value) ? chosen.filter((priority) => priority !== value) : [...chosen, value];
        } else if (kind === "unacknowledged") {
          filter.unacknowledged = !filter.unacknowledged;
        } else if (kind === "text") {
          this.search(objectId, view, redraw);
          return true;
        } else if (kind === "equipment") {
          this.chooseEquipment(objectId, view, redraw);
          return true;
        }
        view.views[objectId] = { ...current2, first: 0, filter: kind === "clear" ? void 0 : filter };
        redraw();
        return true;
      }
      const column = clicked.closest("[data-alarm-sort]")?.getAttribute("data-alarm-sort");
      if (column) {
        const current2 = view.views[objectId]?.sort;
        const reversed = current2?.column === column ? !current2.reversed : false;
        view.views[objectId] = { ...view.views[objectId], first: 0, sort: { column, reversed } };
        redraw();
        return true;
      }
      const page = clicked.closest("[data-page-first]")?.getAttribute("data-page-first");
      if (page !== null && page !== void 0) {
        view.views[objectId] = { ...view.views[objectId], first: Number(page) };
        redraw();
        return true;
      }
      if (clicked.closest("[data-disabled]")) return true;
      const action = clicked.closest("[data-alarm-action]");
      if (action) {
        const alarm = action.getAttribute("data-alarm") ?? "";
        const row = rowOf(view, objectId, alarm);
        void this.act(alarm, action.getAttribute("data-alarm-action") ?? "", void 0, row?.message);
        return true;
      }
      const picked = clicked.closest('[data-part="alarm-row"]')?.getAttribute("data-alarm");
      if (picked) {
        const row = rowOf(view, objectId, picked);
        if (row) this.open(row, view.views[objectId]?.tab ?? "active", view.summary);
        return true;
      }
      return false;
    }
    /** The search: the list keeps the rows holding the text while it is typed. */
    search(objectId, view, redraw) {
      const dialog = this.overlay.dialog("div", "alarm-search");
      dialog.append(element(dialog, "h2", say("Search")));
      const input = dialog.ownerDocument.createElement("input");
      input.type = "search";
      input.name = "search";
      input.value = view.views[objectId]?.filter?.text ?? "";
      input.addEventListener("input", () => {
        const current2 = view.views[objectId] ?? {};
        view.views[objectId] = { ...current2, first: 0, filter: { ...current2.filter, text: input.value } };
        redraw();
      });
      const close = button(dialog, say("Close"), "close");
      close.addEventListener("click", () => this.overlay.closeDialog());
      const actions = element(dialog, "div", "", "hmi-actions");
      actions.append(close);
      dialog.append(input, actions);
      input.focus();
    }
    /** The equipment of the open tab's alarms to choose from, by tag. */
    chooseEquipment(objectId, view, redraw) {
      const current2 = view.views[objectId] ?? {};
      const rows = view.summary?.[current2.tab ?? "active"] ?? [];
      const equipment = [
        ...new Set(rows.map((row) => row.equipment).filter((path) => Boolean(path)))
      ].sort();
      const dialog = this.overlay.dialog("div", "alarm-equipment");
      dialog.append(element(dialog, "h2", say("Equipment")));
      const choices = element(dialog, "div", "", "hmi-actions");
      for (const path of equipment) {
        const choice = button(dialog, path.split(".").pop() ?? path);
        choice.setAttribute("data-equipment", path);
        if (current2.filter?.equipment === path) choice.setAttribute("aria-pressed", "true");
        choice.addEventListener("click", () => {
          this.overlay.closeDialog();
          view.views[objectId] = { ...current2, first: 0, filter: { ...current2.filter, equipment: path } };
          redraw();
        });
        choices.append(choice);
      }
      const close = button(dialog, say("Close"), "close");
      close.addEventListener("click", () => this.overlay.closeDialog());
      choices.append(close);
      dialog.append(choices);
    }
    /** A later outcome from the stream; whether it was one of the actions sent here. */
    receive(command) {
      const what = this.sent.get(command.id);
      if (what === void 0) return false;
      if (command.outcome !== "applied" && command.outcome !== "pending") this.sent.delete(command.id);
      if (command.outcome === "timed-out" || command.outcome === "unknown" || command.outcome === "refused") {
        this.overlay.notice(`${what}: ${outcomeSaid(command.outcome, command.reason)}`);
      }
      return true;
    }
    /** Sends an action for an alarm; without a session it asks to log in instead (§12.4). */
    async act(alarm, action, value, message) {
      if (this.session.readOnly) {
        this.session.showLogin(say("Log in to operate."));
        return;
      }
      const id = commandId();
      const named = ACTIONS[action];
      const what = `${named ? say(named) : action} ${translate(message || alarm)}`;
      this.sent.set(id, what);
      const body = { id, alarm, action };
      if (value !== void 0) body.value = value;
      let answer;
      try {
        answer = await this.host.postJson("/api/hmi/alarm", body);
      } catch {
        this.sent.delete(id);
        this.overlay.notice(`${what}: ${say("The connection was lost before the outcome was known.")}`);
        return;
      }
      const reply = answer.body;
      if (answer.status !== 200 || !reply?.outcome) {
        this.sent.delete(id);
        this.overlay.notice(
          `${what}: ${reply?.error ?? say("The runtime answered {status}.", { status: answer.status })}`
        );
      } else if (reply.outcome === "refused" || reply.outcome === "timed-out") {
        this.sent.delete(id);
        this.overlay.notice(`${what}: ${outcomeSaid(reply.outcome, reply.reason)}`);
      }
    }
    /** The alarm with its facts and the actions its state allows, as the runtime sends them (§5.7, item 4). */
    open(row, tab, summary) {
      const dialog = this.overlay.dialog("div", "alarm");
      dialog.setAttribute("aria-label", row.message || row.path);
      const title = element(dialog, "h2", "", "hmi-alarm-title");
      const mark = element(title, "span", "", "hmi-alarm-mark");
      mark.innerHTML = this.mark(row.priority);
      title.append(mark, element(title, "span", row.message || row.path));
      dialog.append(title);
      const facts = element(dialog, "dl", "", "hmi-facts");
      const fact = (name, content) => {
        facts.append(element(facts, "dt", name), element(facts, "dd", content));
      };
      fact(say("Alarm"), row.path);
      fact(say("Priority"), priorityName(alarmPriority(row.priority)));
      fact(say("State"), rowStateName(row, tab));
      fact(say("Since"), timeOfDay(row.since_ms));
      fact(say("First out"), row.first_out ? say("Yes") : say("No"));
      fact(say("Raised"), String(row.raise_count));
      dialog.append(facts);
      const equipment = button(dialog, say("Show equipment"), "show-equipment");
      equipment.disabled = true;
      const equipmentReason = element(dialog, "p", say("Finding equipment\u2026"), "hmi-hint");
      void equipmentDestination(this.host, row.equipment ?? "").then((url) => {
        if (!dialog.isConnected) return;
        equipment.disabled = !url;
        equipmentReason.textContent = url ? "" : say("Equipment is not shown on a page.");
        equipmentReason.hidden = Boolean(url);
        if (url) equipment.addEventListener("click", () => this.host.navigate(url));
      }).catch(() => {
        if (dialog.isConnected) equipmentReason.textContent = say("Unable to find equipment.");
      });
      const actions = element(dialog, "div", "", "hmi-actions hmi-alarm-actions");
      actions.append(equipment);
      const offer = (action, value, label = ACTIONS[action] ? say(ACTIONS[action]) : action) => {
        const offered = button(dialog, label, action);
        offered.addEventListener("click", () => {
          this.overlay.closeDialog();
          void this.act(row.path, action, value, row.message);
        });
        return offered;
      };
      const allowed = row.actions ?? [];
      for (const action of allowed) {
        if (action !== "shelve" && action !== "comment") actions.append(offer(action));
      }
      if (actions.childElementCount > 0) dialog.append(actions);
      dialog.append(equipmentReason);
      if (allowed.includes("shelve")) {
        dialog.append(element(dialog, "p", say("Shelve for"), "hmi-hint"));
        const shelve = element(dialog, "div", "", "hmi-actions hmi-shelve");
        for (const seconds of summary?.shelve_for ?? []) {
          const offered = offer("shelve", seconds, durationName(seconds));
          offered.setAttribute("data-shelve", String(seconds));
          shelve.append(offered);
        }
        dialog.append(shelve);
      }
      const footer = element(dialog, "div", "", "hmi-actions");
      const close = button(dialog, say("Close"), "cancel");
      close.addEventListener("click", () => this.overlay.closeDialog());
      footer.append(close);
      if (allowed.includes("comment")) {
        const comment = element(dialog, "label", say("Comment"));
        const input = dialog.ownerDocument.createElement("input");
        input.name = "comment";
        if (summary?.comment_max_chars) input.maxLength = summary.comment_max_chars;
        input.setAttribute("autocomplete", "off");
        comment.append(input);
        dialog.append(comment);
        const add = button(dialog, say("Add comment"), "comment");
        add.addEventListener("click", () => {
          this.overlay.closeDialog();
          void this.act(row.path, "comment", input.value.trim(), row.message);
        });
        footer.append(add);
      }
      dialog.append(footer);
    }
  };
  function rowOf(view, objectId, path) {
    const tab = view.views[objectId]?.tab ?? "active";
    return view.summary?.[tab].find((row) => row.path === path);
  }

  // src/hmiBuilder/display/session.ts
  var SessionControl = class {
    constructor(host2, overlay, changed, slot) {
      this.host = host2;
      this.overlay = overlay;
      this.changed = changed;
      this.slot = slot;
    }
    /** Reads the session the page opened with. */
    async start() {
      try {
        const answer = await this.host.fetchJson("/api/hmi/session");
        this.session = answer.status === 200 ? sessionOf(answer.body) : void 0;
      } catch {
        this.session = void 0;
      }
      this.showBar();
    }
    /** Without a session the controls cannot be used (§12.4). */
    get readOnly() {
      return this.session === void 0;
    }
    /** The session's role, if there is a session. */
    get role() {
      return this.session?.role;
    }
    /** The session the stream reports; only a session that ends asks to log in again. */
    receive(session) {
      const had = this.session !== void 0;
      this.session = session ?? void 0;
      this.showBar();
      if (had && !this.session) this.showLogin(say("Your session ended. Log in to operate."));
    }
    showLogin(message, peer) {
      const form = this.overlay.dialog("form", "login");
      form.append(element(form, "h2", peer ? `${say("Log in")} \xB7 ${peer}` : say("Log in")));
      form.append(element(form, "p", message, "hmi-hint"));
      const user = labelledInput(form, say("User"), "user", "username");
      const password = labelledInput(form, say("Password"), "password", "current-password");
      password.type = "password";
      const error = element(form, "p", "", "hmi-error");
      form.append(error);
      const actions = element(form, "div", "", "hmi-actions");
      const cancel = button(form, say("Not now"), "cancel");
      cancel.type = "button";
      cancel.addEventListener("click", () => this.overlay.closeDialog());
      const submit = button(form, say("Log in"));
      submit.type = "submit";
      actions.append(cancel, submit);
      form.append(actions);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        void this.login(user.value, password.value, error, peer);
      });
      user.focus();
    }
    async login(user, password, error, peer) {
      try {
        const answer = await this.host.postJson("/api/hmi/login", { user, password, ...peer ? { peer } : {} });
        const session = answer.status === 200 ? sessionOf(answer.body) : void 0;
        if (!session) {
          error.textContent = answer.body?.error ?? say("The login was refused.");
          return;
        }
        if (!peer) this.session = session;
        this.overlay.closeDialog();
        this.showBar();
        this.changed();
      } catch {
        error.textContent = say("The runtime cannot be reached.");
      }
    }
    async logout() {
      try {
        await this.host.postJson("/api/hmi/logout", {});
      } catch {
      }
      this.session = void 0;
      this.showBar();
      this.changed();
    }
    /** The session bar: the user and role with Log out, or Log in (§12.4). */
    showBar() {
      const home = this.slot ?? this.overlay.layer;
      home.querySelector('[data-part="session"]')?.remove();
      const bar = element(home, "div", "", this.slot ? "hmi-session hmi-session-inline" : "hmi-session");
      bar.setAttribute("data-part", "session");
      if (this.session) {
        bar.append(
          element(bar, "span", this.session.user, "hmi-user"),
          element(bar, "span", roleName(this.session.role), "hmi-role")
        );
        const out = button(bar, say("Log out"), "logout");
        out.addEventListener("click", () => void this.logout());
        bar.append(out);
      } else {
        const login = button(bar, say("Log in"), "login");
        login.addEventListener("click", () => this.showLogin(say("Log in to operate.")));
        bar.append(login);
      }
      if (this.slot) home.append(bar);
      else home.prepend(bar);
    }
  };
  function sessionOf(body) {
    const session = body;
    return typeof session?.user === "string" && typeof session.role === "string" ? { user: session.user, role: session.role } : void 0;
  }
  function roleName(role) {
    return role.charAt(0).toUpperCase() + role.slice(1);
  }

  // src/hmiBuilder/display/settingValues.ts
  function settingLabel(setting) {
    return setting.unit ? `${setting.title} (${setting.unit})` : setting.title;
  }
  function settingText(value) {
    return value === null ? "" : String(value);
  }
  function limit(value, unit) {
    return unit ? `${value} ${unit}` : String(value);
  }
  function enteredValue(setting, entered) {
    if (setting.choices?.length) {
      const choice = setting.choices.find((candidate) => String(candidate.value) === String(entered));
      if (!choice) throw new Error(`${setting.title}: choose one of its values.`);
      return choice.value;
    }
    if (setting.kind === "bool") return entered === true || entered === "true";
    if (setting.kind !== "integer" && setting.kind !== "real" && setting.kind !== "time") return String(entered);
    const text2 = String(entered).trim().replace(",", ".");
    const number = Number(text2);
    if (text2 === "" || !Number.isFinite(number)) throw new Error(`${setting.title}: enter a number.`);
    if (setting.kind === "integer" && !Number.isInteger(number))
      throw new Error(`${setting.title}: enter a whole number.`);
    if (setting.min !== void 0 && number < setting.min)
      throw new Error(`${setting.title}: ${number} is below the minimum ${limit(setting.min, setting.unit)}.`);
    if (setting.max !== void 0 && number > setting.max)
      throw new Error(`${setting.title}: ${number} is above the maximum ${limit(setting.max, setting.unit)}.`);
    return number;
  }
  function settingChanged(setting, value) {
    return value !== setting.value;
  }

  // src/hmiBuilder/display/settings.ts
  var SETTINGS_CSS = `
.hmi-dialog.hmi-settings-form { width: min(560px, 92vw); max-width: none; max-height: min(86vh, 760px); display: flex; flex-direction: column; gap: 0; padding: 0; overflow: hidden; }
.hmi-settings-form > h2 { padding: 20px 24px 12px; }
.hmi-settings-fields { flex: 1 1 auto; min-height: 0; overflow: auto; display: grid; align-content: start; gap: 16px; padding: 4px 24px 18px; }
.hmi-settings-field { display: grid; gap: 6px; }
.hmi-settings-field span { font-weight: 600; font-size: 14px; color: var(--hmi-text); }
.hmi-settings-field small { color: var(--hmi-muted); font-size: 13px; line-height: 1.4; }
.hmi-settings-field input, .hmi-settings-field select { box-sizing: border-box; min-height: 44px; width: 100%; color: var(--hmi-text); background: var(--hmi-page); border: 1px solid var(--hmi-border); border-radius: 10px; padding: 8px 12px; font: 500 16px var(--hmi-font); }
.hmi-settings-field input[type=checkbox] { width: 22px; min-height: 22px; justify-self: start; }
.hmi-settings-field [aria-invalid=true] { border-color: var(--hmi-alarm); }
.hmi-settings-footer { position: sticky; bottom: 0; display: flex; align-items: center; gap: 10px; padding: 14px 24px; border-top: 1px solid var(--hmi-border); background: var(--hmi-surface); }
.hmi-settings-result { flex: 1; min-width: 0; margin: 0; color: var(--hmi-muted); font-size: 13px; }
.hmi-settings-result[data-refused] { color: var(--hmi-alarm); }
`;
  function mayChangeSettings(role) {
    return role === "engineer" || role === "admin";
  }
  function field(form, setting) {
    const document2 = form.ownerDocument;
    if (setting.choices?.length) {
      const select = document2.createElement("select");
      select.name = setting.member;
      const options = [...setting.choices];
      if (setting.value !== null && !options.some((choice) => String(choice.value) === settingText(setting.value))) {
        options.push({ value: setting.value, title: settingText(setting.value) });
      }
      for (const choice of options) {
        const option = document2.createElement("option");
        option.value = String(choice.value);
        option.textContent = choice.title;
        select.append(option);
      }
      select.value = settingText(setting.value);
      return select;
    }
    const input = document2.createElement("input");
    input.name = setting.member;
    if (setting.kind === "bool") {
      input.type = "checkbox";
      input.checked = setting.value === true;
      return input;
    }
    input.type = "text";
    input.value = settingText(setting.value);
    input.setAttribute("autocomplete", "off");
    if (setting.kind === "integer") input.inputMode = "numeric";
    else if (setting.kind === "real" || setting.kind === "time") input.inputMode = "decimal";
    return input;
  }
  function read(input) {
    return input.tagName === "INPUT" && input.type === "checkbox" ? input.checked : input.value;
  }
  function write(input, entered) {
    if (typeof entered === "boolean") input.checked = entered;
    else input.value = entered;
  }
  var EquipmentSettings = class {
    constructor(host2, overlay, session) {
      this.host = host2;
      this.overlay = overlay;
      this.session = session;
    }
    /** Opens the Settings dialog of a panel popup, `faceplate:<page>/<object id>`, titled by its
     * equipment ("P101 — Pump"). */
    async open(faceplate, title) {
      if (this.session.readOnly) {
        this.session.showLogin(say("Log in to change settings."));
        return;
      }
      if (!mayChangeSettings(this.session.role)) {
        this.overlay.notice(say("This needs the {role} role.", { role: say("Engineer") }));
        return;
      }
      const form = this.overlay.dialog("form", "equipment-settings");
      form.classList.add("hmi-settings-form");
      form.append(element(form, "h2", `${title} \xB7 ${say("Settings")}`));
      const fields = element(form, "div", "", "hmi-settings-fields");
      fields.setAttribute("data-part", "settings-fields");
      fields.append(element(fields, "p", say("Loading settings\u2026"), "hmi-hint"));
      const footer = element(form, "div", "", "hmi-settings-footer");
      footer.setAttribute("data-part", "settings-footer");
      const result = element(footer, "p", "", "hmi-settings-result");
      result.setAttribute("data-part", "settings-result");
      result.setAttribute("role", "status");
      const cancel = button(footer, say("Cancel"), "cancel");
      cancel.addEventListener("click", () => this.overlay.closeDialog());
      const drafting = () => {
        cancel.textContent = say("Cancel");
      };
      form.addEventListener("input", drafting);
      form.addEventListener("change", drafting);
      const apply = button(footer, say("Apply"), "apply");
      apply.type = "submit";
      apply.disabled = true;
      const keep = this.host.keepSettings ? button(footer, say("Keep in project"), "keep") : void 0;
      if (keep) keep.disabled = true;
      footer.append(result, cancel, ...keep ? [keep] : [], apply);
      form.append(fields, footer);
      let shown = [];
      let instance;
      const inputs = /* @__PURE__ */ new Map();
      let busy = false;
      const report = (text2, refused = false) => {
        result.textContent = text2;
        if (refused) result.setAttribute("data-refused", "true");
        else result.removeAttribute("data-refused");
      };
      const send = async (body) => {
        const answer = await this.host.postJson("/api/hmi/settings", { faceplate, ...body });
        const reply = answer.body;
        if (!reply || typeof reply !== "object") throw new Error("no answer");
        return reply;
      };
      const render = (settings, draft) => {
        shown = settings;
        inputs.clear();
        fields.replaceChildren();
        for (const setting of settings) {
          const label = element(fields, "label", "", "hmi-settings-field");
          label.setAttribute("data-member", setting.member);
          label.append(element(label, "span", settingLabel(setting)));
          const input = field(form, setting);
          const kept = draft.get(setting.member);
          if (kept !== void 0) write(input, kept);
          label.append(input);
          if (setting.help) label.append(element(label, "small", setting.help));
          fields.append(label);
          inputs.set(setting.member, input);
        }
        apply.disabled = settings.length === 0;
        if (keep) keep.disabled = settings.length === 0;
      };
      const drafted = () => {
        const draft = /* @__PURE__ */ new Map();
        for (const setting of shown) {
          const input = inputs.get(setting.member);
          if (!input) continue;
          const entered = read(input);
          let edited = true;
          try {
            edited = settingChanged(setting, enteredValue(setting, entered));
          } catch {
          }
          if (edited) draft.set(setting.member, entered);
        }
        return draft;
      };
      const load = async (draft) => {
        const reply = await send({ action: "read" });
        if (!form.isConnected) return;
        if (!reply.ok || !reply.settings) {
          report(reply.error ?? say("The settings cannot be read."), true);
          return;
        }
        instance = reply.instance;
        render(reply.settings, draft);
      };
      keep?.addEventListener("click", () => {
        const keepSettings = this.host.keepSettings?.bind(this.host);
        if (busy || !instance || !keepSettings) return;
        const values = Object.fromEntries(shown.map((setting) => [setting.member, setting.value]));
        busy = true;
        keep.disabled = true;
        report(say("Keeping in the project\u2026"));
        void (async () => {
          try {
            const answer = (await keepSettings(instance, values)).body;
            if (answer?.ok)
              report(answer.kept?.length ? say("Kept in the project.") : say("The project already has these values."));
            else report(answer?.error ?? say("The values could not be kept in the project."), true);
          } catch {
            report(say("The values could not be kept in the project."), true);
          } finally {
            busy = false;
            keep.disabled = shown.length === 0;
          }
        })();
      });
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        if (busy || !form.isConnected) return;
        const changes = {};
        const expected = {};
        inputs.forEach((input) => input.removeAttribute("aria-invalid"));
        for (const setting of shown) {
          const input = inputs.get(setting.member);
          if (!input) continue;
          let value;
          try {
            value = enteredValue(setting, read(input));
          } catch (error) {
            input.setAttribute("aria-invalid", "true");
            input.focus();
            report(error instanceof Error ? error.message : String(error), true);
            return;
          }
          if (settingChanged(setting, value)) {
            changes[setting.member] = value;
            expected[setting.member] = setting.value;
          }
        }
        if (Object.keys(changes).length === 0) {
          report(say("There are no changes to apply."));
          return;
        }
        busy = true;
        apply.disabled = true;
        report(say("Applying\u2026"));
        void (async () => {
          try {
            const reply = await send({ action: "apply", changes, expected });
            if (!form.isConnected) return;
            if (reply.ok && reply.settings) {
              instance = reply.instance ?? instance;
              render(reply.settings, /* @__PURE__ */ new Map());
              report(say("Applied"));
              cancel.textContent = say("Close");
              return;
            }
            if (reply.changed) await load(drafted());
            const input = reply.field ? inputs.get(reply.field) : void 0;
            input?.setAttribute("aria-invalid", "true");
            input?.focus();
            report(reply.error ?? say("The settings were not applied."), true);
          } catch {
            report(`${say("The runtime cannot be reached.")} ${say("Check the values before another change.")}`, true);
          } finally {
            busy = false;
            apply.disabled = shown.length === 0;
          }
        })();
      });
      try {
        await load(/* @__PURE__ */ new Map());
      } catch {
        report(say("The runtime cannot be reached."), true);
      }
    }
  };

  // src/hmiBuilder/display/operate.ts
  var Operation = class {
    constructor(container, host2, page, bundle, redraw, liveAt, sessionChanged = () => void 0, sessionSlot, alarmMark) {
      this.host = host2;
      this.bundle = bundle;
      this.redraw = redraw;
      /** Popups' command surfaces, which hear the same command events (§4.9). */
      this.others = /* @__PURE__ */ new Set();
      this.overlay = new Overlay(container, host2);
      this.formats = page.formats;
      this.session = new SessionControl(
        host2,
        this.overlay,
        () => {
          this.redraw();
          sessionChanged();
        },
        sessionSlot
      );
      this.settings = new EquipmentSettings(host2, this.overlay, this.session);
      this.commands = new CommandSurface(host2, page, bundle, this.session, this.overlay, redraw, liveAt);
      this.alarms = new AlarmActions(host2, this.session, this.overlay, alarmMark);
    }
    /** Reads the session the page opened with. */
    async start() {
      await this.session.start();
    }
    /** Without a session the controls cannot be used (§12.4). */
    get readOnly() {
      return this.session.readOnly;
    }
    /** The latest outcome on each control. */
    states() {
      return this.commands.states();
    }
    /** The values being entered on controls. */
    drafts() {
      return this.commands.drafts();
    }
    /** The page's controls the signed-in role may not use (§5.9 item 5). */
    locked() {
      return this.commands.locked();
    }
    /** A command surface for a popup, sharing the session and the overlay (§4.9). */
    popupSurface(popup, redraw, liveAt) {
      const page = popup.name.startsWith("faceplate:") ? popup.name : `popup:${popup.name}`;
      const surface = new CommandSurface(
        this.host,
        { name: page, model: popup.model, roles: popup.roles, formats: popup.formats ?? this.formats },
        this.bundle,
        this.session,
        this.overlay,
        redraw,
        liveAt
      );
      this.others.add(surface);
      return surface;
    }
    /** Forgets a closed popup's commands. */
    closePopupSurface(surface) {
      this.others.delete(surface);
    }
    /** A stream message about the session or a command; whether it was one. */
    receive(message) {
      let handled = false;
      if ("session" in message) {
        handled = true;
        this.session.receive(message.session);
      }
      if (typeof message.bundle === "string") {
        handled = true;
        if (message.bundle !== this.bundle) this.overlay.showReload();
      }
      if (message.command) {
        handled = true;
        const command = message.command;
        if (!this.commands.receive(command) && !this.alarms.receive(command)) {
          for (const other of this.others) if (other.receive(command)) break;
        }
      }
      if (handled) this.redraw();
      return handled;
    }
    /** A click on a drawn object; `option` is the mode or option clicked inside it. */
    click(objectId, option) {
      this.commands.click(objectId, option);
    }
    /** A press on a drawn object at a point of the page; whether it started dragging a slider. */
    press(objectId, at) {
      return this.commands.press(objectId, at);
    }
    /** The pointer moved while a slider is dragged. */
    drag(at) {
      this.commands.drag(at);
    }
    /** The pointer was released: a dragged slider writes its value. */
    release(at) {
      this.commands.release(at);
    }
  };

  // src/hmiBuilder/drawing/placed.ts
  var PLACED_HEADER = 44;

  // src/hmiBuilder/display/placedFaceplates.ts
  var PlacedFaceplates = class {
    constructor(placed, operation, redraw, liveAt, page) {
      this.page = page;
      this.placements = /* @__PURE__ */ new Map();
      /** The placements whose faceplate the project lacks. */
      this.lacking = [];
      for (const [id, data] of Object.entries(placed)) {
        if (data)
          this.placements.set(id, {
            data,
            commands: operation.popupSurface({ name: data.name, model: data.model, roles: data.roles }, redraw, liveAt)
          });
        else this.lacking.push(id);
      }
    }
    /** What the drawing needs of each placement. */
    frame() {
      const frame = Object.fromEntries(this.lacking.map((id) => [id, null]));
      for (const [id, { data, commands }] of this.placements) {
        frame[id] = {
          model: data.model,
          commands: commands.states(),
          drafts: commands.drafts(),
          locked: commands.locked()
        };
      }
      return frame;
    }
    /** A click inside a placement, on one of its objects; whether it was inside one. */
    click(target, option) {
      const found = this.within(target);
      if (!found) return false;
      if (found.object) found.placement.commands.click(found.object, option);
      return true;
    }
    /** A press inside a placement, at a point on the page; whether one of its controls took it. */
    press(target, at) {
      const found = this.within(target);
      if (!found?.object) return false;
      const taken = found.placement.commands.press(found.object, this.inner(found.id, at));
      this.dragging = taken ? found.id : void 0;
      return taken;
    }
    drag(at) {
      if (this.dragging) this.placements.get(this.dragging)?.commands.drag(this.inner(this.dragging, at));
    }
    release(at) {
      const id = this.dragging;
      this.dragging = void 0;
      if (id) this.placements.get(id)?.commands.release(this.inner(id, at));
    }
    /** The placement a target lies in, and the object of its faceplate the target belongs to. */
    within(target) {
      const box = target?.closest?.("[data-placed]");
      const id = box?.getAttribute("data-placed");
      const placement = id ? this.placements.get(id) : void 0;
      if (!box || !id || !placement) return void 0;
      const object = target?.closest?.("[data-object]");
      return {
        id,
        placement,
        object: object && box.contains(object) ? object.getAttribute("data-object") ?? void 0 : void 0
      };
    }
    /** A point on the page in a placement's own coordinates: the card keeps the faceplate's
     * proportions, centred in its object, with the header above the faceplate's objects. */
    inner(id, at) {
      const object = this.page().object.find((candidate) => candidate.id === id);
      const model = this.placements.get(id)?.data.model;
      if (!object || !model) return at;
      const [width, height] = object.size;
      const [faceplateWidth, faceplateHeight] = model.size;
      const total = faceplateHeight + PLACED_HEADER;
      const scale = Math.min(width / faceplateWidth, height / total);
      const left = object.at[0] + (width - faceplateWidth * scale) / 2;
      const top = object.at[1] + (height - total * scale) / 2;
      return { x: (at.x - left) / scale, y: (at.y - top) / scale - PLACED_HEADER };
    }
  };

  // src/hmiBuilder/display/shared.ts
  var DEFAULT_SPAN_S = 300;
  function socketUrl(location, query) {
    const scheme = location.protocol === "https:" ? "wss:" : "ws:";
    return `${scheme}//${location.host}/ws/hmi?${query}`;
  }
  function parseMessage(data) {
    if (typeof data !== "string") return void 0;
    try {
      const message = JSON.parse(data);
      return typeof message === "object" && message !== null ? message : void 0;
    } catch {
      return void 0;
    }
  }
  function designPoint(surface, size, event) {
    const svg = surface.querySelector("svg") ?? surface;
    const box = svg.getBoundingClientRect();
    const scale = Math.min(box.width / size[0], box.height / size[1]) || 1;
    const left = box.left + (box.width - size[0] * scale) / 2;
    const top = box.top + (box.height - size[1] * scale) / 2;
    return { x: (event.clientX - left) / scale, y: (event.clientY - top) / scale };
  }
  function trendSpans(page, placed = []) {
    const spans = {};
    const objects = [page, ...placed].flatMap((model) => model.object);
    for (const object of objects.filter((candidate) => candidate.type === "trend")) {
      const spanMs = (object.span ?? DEFAULT_SPAN_S) * 1e3;
      for (const [name, binding] of Object.entries(object.bind)) {
        if (!NOT_PENS.includes(name)) spans[binding.path] = Math.max(spans[binding.path] ?? 0, spanMs);
      }
    }
    return spans;
  }

  // src/hmiBuilder/display/popups.ts
  var FACEPLATE = "faceplate:";
  var RECONNECT_MS = 2e3;
  var POPUP_CSS = `
.hmi-popup-backdrop { position: fixed; inset: 0; z-index: 15; display: grid; place-items: center; background: rgba(0, 0, 0, 0.5); backdrop-filter: blur(2px); }
.hmi-popup { box-sizing: border-box; display: flex; flex-direction: column; max-width: 92vw; border-radius: 16px; overflow: hidden; background: var(--hmi-surface); border: 1px solid var(--hmi-border); box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5); }
.hmi-popup-header { display: flex; align-items: center; gap: 8px; padding: 10px 10px 10px 18px; border-bottom: 1px solid var(--hmi-border); }
.hmi-popup-heading { flex: 1; min-width: 0; }
.hmi-popup-header h2 { margin: 0; font: 650 16px var(--hmi-font); color: var(--hmi-text); }
.hmi-popup-header p { margin: 2px 0 0; font: 500 14px var(--hmi-font); color: var(--hmi-muted); }
.hmi-popup-header button { min-width: 36px; height: 36px; border-radius: 10px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 14px var(--hmi-font); }
.hmi-popup-surface > svg { display: block; width: 100%; height: 100%; }
.hmi-pinned { position: fixed; right: 24px; bottom: 24px; z-index: 12; display: flex; flex-direction: column-reverse; gap: 12px; }
`;
  var Popups = class {
    constructor(root, host2, drawing, operation, follow) {
      this.root = root;
      this.host = host2;
      this.drawing = drawing;
      this.operation = operation;
      this.follow = follow;
      this.windows = [];
      this.disposed = false;
      this.keydown = (event) => {
        if (event.key === "Escape") this.windows[this.windows.length - 1]?.close();
      };
      root.ownerDocument.addEventListener("keydown", this.keydown);
    }
    /** Ends every popup with its owning display, including pending opens. */
    dispose() {
      this.disposed = true;
      this.root.ownerDocument.removeEventListener("keydown", this.keydown);
      [...this.windows].forEach((window2) => window2.close());
      this.pinnedArea?.remove();
    }
    /** Opens a popup by name, unless it is open already. */
    async open(name) {
      await this.show(name, `/api/hmi/popup?name=${encodeURIComponent(name)}`, name);
    }
    /** Opens the faceplate of an object, named `<page>/<object id>` (or `popup:<name>/<object id>`),
     * unless it is open already; `name` is the faceplate's, for a notice (§8.7, item 5). */
    async openFaceplate(source, name) {
      await this.show(`${FACEPLATE}${source}`, `/api/hmi/popup?faceplate=${encodeURIComponent(source)}`, name);
    }
    async show(address, url, name) {
      if (this.disposed || this.windows.some((window3) => window3.name === address)) return;
      let answer;
      try {
        answer = await this.host.fetchJson(url);
      } catch {
        if (this.disposed) return;
        this.operation.overlay.notice(
          say('The popup "{name}" cannot be opened: the runtime cannot be reached.', { name })
        );
        return;
      }
      if (this.disposed) return;
      const body = answer.body;
      const popup = body?.popup;
      if (answer.status !== 200 || !popup) {
        this.operation.overlay.notice(say('The popup "{name}" cannot be opened.', { name }));
        return;
      }
      this.drawing.registerSymbols(body?.symbols ?? []);
      const window2 = new PopupWindow(this, popup.name, popup.model, popup.roles ?? {}, body?.formats);
      this.windows.push(window2);
    }
    /** Every popup draws again, for stale ages and the session. */
    redraw() {
      this.windows.forEach((window2) => window2.redraw());
    }
    /** The area pinned popups stay in. */
    pinned() {
      if (!this.pinnedArea) {
        this.pinnedArea = this.root.ownerDocument.createElement("div");
        this.pinnedArea.className = "hmi-pinned";
        this.root.append(this.pinnedArea);
      }
      return this.pinnedArea;
    }
    forget(window2) {
      this.windows.splice(this.windows.indexOf(window2), 1);
    }
    get context() {
      return { root: this.root, host: this.host, drawing: this.drawing, operation: this.operation, follow: this.follow };
    }
  };
  var PopupWindow = class {
    constructor(popups, name, model, roles, formats) {
      this.popups = popups;
      this.name = name;
      this.model = model;
      this.formats = formats;
      /** The alarm list and records the popup's stream sends (specification 40, §5.1). */
      this.alarms = new AlarmView();
      this.pending = false;
      this.closed = false;
      this.dragging = false;
      this.cleanup = [];
      const { root, host: host2, operation, drawing } = popups.context;
      this.valuePaint = new ValuePaintQueue(
        (callback) => host2.schedule(callback),
        (element2, before, after) => updateNode(element2, before, after, drawing.renderToString)
      );
      const document2 = root.ownerDocument;
      this.backdrop = document2.createElement("div");
      this.backdrop.className = "hmi-popup-backdrop";
      this.backdrop.setAttribute("data-part", "popup-backdrop");
      this.window = document2.createElement("section");
      this.window.className = "hmi-popup";
      this.window.setAttribute("data-part", "popup");
      this.window.setAttribute("role", "dialog");
      this.window.setAttribute("aria-label", model.title);
      this.window.style.width = `min(${model.size[0]}px, 92vw)`;
      const header = document2.createElement("header");
      header.className = "hmi-popup-header";
      this.header = header;
      const heading = document2.createElement("div");
      heading.className = "hmi-popup-heading";
      const title = document2.createElement("h2");
      title.setAttribute("data-part", "popup-title");
      title.textContent = model.title;
      heading.append(title);
      if (model.description) {
        const description = document2.createElement("p");
        description.setAttribute("data-part", "popup-description");
        description.textContent = model.description;
        heading.append(description);
      }
      header.append(heading);
      this.settingsButton = model.settings_instance ? this.button(say("Settings"), "settings", say("Equipment settings"), () => {
        void operation.settings.open(name, model.title);
      }) : void 0;
      this.showSettingsButton();
      if (model.pinnable)
        header.append(this.button(say("Pin"), "pin", say("Keep open beside the page"), () => this.pin()));
      header.append(this.button("\u2715", "close", say("Close"), () => this.close()));
      this.surface = document2.createElement("div");
      this.surface.className = "hmi-popup-surface";
      this.surface.style.aspectRatio = `${model.size[0]} / ${model.size[1]}`;
      this.window.append(header, this.surface);
      this.backdrop.append(this.window);
      root.append(this.backdrop);
      this.store = new LiveValueStore({ trendSpansMs: trendSpans(model) });
      this.trends = new TrendActions(
        () => model,
        this.store,
        (url) => host2.fetchJson(url),
        this.surface,
        () => host2.now()
      );
      this.commands = operation.popupSurface(
        { name, model, roles, formats },
        () => this.redraw(),
        (path) => this.store.frame(host2.now()).values[path]
      );
      this.surface.addEventListener("click", (event) => this.clicked(event));
      this.surface.addEventListener("pointerdown", (event) => {
        operation.overlay.touched = event.pointerType === "touch";
        const id = event.target?.closest?.("[data-object]")?.getAttribute("data-object");
        if (id && this.trends.press(id, this.at(event), event.target)) {
          event.preventDefault();
          this.redraw();
          return;
        }
        this.dragging = id !== void 0 && id !== null && this.commands.press(id, this.at(event));
        if (this.dragging) event.preventDefault();
      });
      this.surface.addEventListener(
        "wheel",
        (event) => {
          const id = event.target?.closest?.("[data-object]")?.getAttribute("data-object");
          if (id && this.trends.wheel(id, this.at(event), event.deltaY)) {
            event.preventDefault();
            this.redraw();
          }
        },
        { passive: false }
      );
      const pointer = document2.defaultView ?? document2;
      const move = (event) => {
        const over = event.target?.closest?.("[data-object]")?.getAttribute("data-object") ?? void 0;
        if (this.trends.drag(this.at(event)) || this.trends.hover(over, this.at(event)))
          this.redraw();
        if (this.dragging) this.commands.drag(this.at(event));
      };
      const release = (event) => {
        if (this.trends.release(this.at(event))) this.redraw();
        if (!this.dragging) return;
        this.dragging = false;
        this.commands.release(this.at(event));
      };
      pointer.addEventListener("pointermove", move);
      pointer.addEventListener("pointerup", release);
      this.cleanup.push(
        () => pointer.removeEventListener("pointermove", move),
        () => pointer.removeEventListener("pointerup", release)
      );
      this.connect();
      this.redraw();
      void loadRecords(host2, model, this.alarms).then(() => this.redraw());
      void loadHistory((url) => host2.fetchJson(url), trendSpans(model), this.store).then(() => this.redraw());
    }
    /** Draws again before the next paint; the header follows the session. */
    redraw() {
      this.showSettingsButton();
      if (this.pending || this.closed) return;
      this.pending = true;
      const { host: host2 } = this.popups.context;
      host2.schedule(() => {
        this.pending = false;
        if (!this.closed) this.render();
      });
    }
    /** The Settings button stands in the header, after the title, only while an Engineer is
     * signed in; Viewers and Operators do not see it (§21.4 item 1). */
    showSettingsButton() {
      const button2 = this.settingsButton;
      if (!button2) return;
      const shown = mayChangeSettings(this.popups.context.operation.session.role);
      const placed = button2.parentElement === this.header;
      if (shown && !placed) this.header.querySelector(".hmi-popup-heading")?.after(button2);
      else if (!shown && placed) button2.remove();
    }
    /** Pinned, the popup stays open beside the page, which can be used again. */
    pin() {
      this.popups.pinned().append(this.window);
      this.backdrop.remove();
      this.window.setAttribute("data-pinned", "true");
      this.window.querySelector('[data-action="pin"]')?.remove();
    }
    close() {
      if (this.closed) return;
      this.closed = true;
      this.valuePaint.dispose();
      this.dragging = false;
      this.cleanup.forEach((remove) => remove());
      if (this.socket) {
        this.socket.onmessage = null;
        this.socket.onclose = null;
        this.socket.close();
      }
      this.backdrop.remove();
      this.window.remove();
      this.popups.context.operation.closePopupSurface(this.commands);
      this.popups.forget(this);
    }
    render() {
      const { host: host2, drawing, operation } = this.popups.context;
      const frame = {
        size: this.model.size,
        formats: this.formats,
        ...this.store.frame(host2.now()),
        commands: this.commands.states(),
        drafts: this.commands.drafts(),
        readOnly: operation.readOnly,
        locked: this.commands.locked(),
        trendViews: this.trends.views,
        asset: host2.asset ?? assetUrl,
        ...displaySpeech(),
        alarms: this.alarms.frame(this.store.isStale(host2.now()))
      };
      this.drawn = patchSurface(
        this.surface,
        drawing.drawPage(this.model, frame),
        drawing.renderToString,
        this.drawn,
        this.valuePaint
      );
      this.valuePaint.flush();
    }
    connect() {
      const { host: host2 } = this.popups.context;
      const query = this.name.startsWith(FACEPLATE) ? `faceplate=${encodeURIComponent(this.name.slice(FACEPLATE.length))}` : `popup=${encodeURIComponent(this.name)}`;
      const opened = host2.openSocket(socketUrl(host2.location, query));
      this.socket = opened;
      opened.onmessage = (event) => {
        const message = parseMessage(event.data);
        if (!message || "session" in message || "command" in message || "bundle" in message || "simulation" in message)
          return;
        if ("alarms" in message || "events" in message) {
          this.alarms.receive(message);
          this.redraw();
          return;
        }
        this.store.receive(message, host2.now());
        this.redraw();
      };
      opened.onclose = () => {
        this.store.disconnected(host2.now());
        this.redraw();
        host2.later(RECONNECT_MS, () => {
          if (!this.closed) this.connect();
        });
      };
    }
    clicked(event) {
      const clicked = event.target;
      const target = clicked?.closest?.("[data-object]");
      const id = target?.getAttribute("data-object");
      if (!id) return;
      if (this.trends.click(clicked, id, () => this.redraw())) return;
      if (this.popups.context.operation.alarms.click(clicked, id, this.alarms, () => this.redraw())) return;
      const follow = target?.getAttribute("data-click");
      if (follow) {
        this.popups.context.follow(follow, id, `popup:${this.name}`);
        return;
      }
      const option = clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? void 0;
      this.commands.click(id, option);
    }
    at(event) {
      return designPoint(this.surface, this.model.size, event);
    }
    button(text2, action, label, run2) {
      const created = this.window.ownerDocument.createElement("button");
      created.type = "button";
      created.textContent = text2;
      created.title = label;
      created.setAttribute("aria-label", label);
      created.setAttribute("data-action", action);
      created.addEventListener("click", run2);
      return created;
    }
  };

  // src/hmiBuilder/display/runtimeDisplay.ts
  var FONT_URL = "/hmi/assets/fonts/InterVariable.woff2";
  var RECONNECT_MS2 = 2e3;
  var REDRAW_MS = 1e3;
  var DISPLAY_CSS = `
@keyframes hmi-blink { 50% { opacity: 0.25; } }
.hmi-blink { animation: hmi-blink 1s step-end infinite; }
.hmi-surface { width: 100%; height: 100%; }
.hmi-surface > svg { display: block; width: 100%; height: 100%; }
.hmi-surface.hmi-scroll { overflow: auto; }
.hmi-surface.hmi-scroll > svg { width: auto; height: auto; }
.hmi-message { box-sizing: border-box; min-height: 100%; display: grid; place-items: center; padding: 32px; }
.hmi-message section { max-width: 720px; padding: 28px 32px; border-radius: 16px; background: var(--hmi-surface); border: 1px solid var(--hmi-border); }
.hmi-message h1 { margin: 0 0 12px; font: 650 22px var(--hmi-font); color: var(--hmi-text); }
.hmi-message p { margin: 6px 0 0; font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; color: var(--hmi-muted); }
`;
  var LANGUAGE_KEY = "trust-hmi-language";
  var THEME_KEY = "trust-hmi-theme";
  async function startDisplay(container, host2, drawing, signal) {
    if (signal?.aborted) return;
    const requested = pageNameOf(host2.location.pathname);
    const pinned = new URLSearchParams(host2.location.search ?? "").get("lang") ?? void 0;
    const asked = pinned ?? host2.recall?.(LANGUAGE_KEY);
    const query = [
      requested !== void 0 ? `name=${encodeURIComponent(requested)}` : void 0,
      asked ? `lang=${encodeURIComponent(asked)}` : void 0
    ].filter((part) => part !== void 0);
    const url = `/api/hmi/page${query.length > 0 ? `?${query.join("&")}` : ""}`;
    let answer;
    try {
      answer = await host2.fetchJson(url);
    } catch (error) {
      if (signal?.aborted) return;
      showMessage(container, drawing, say("The runtime cannot be reached"), [String(error)]);
      return;
    }
    if (signal?.aborted) return;
    const data = answer.body;
    setSpeech(data?.texts ?? {}, data?.language ?? "en");
    container.ownerDocument.documentElement.setAttribute("lang", data?.language ?? "en");
    if (answer.status !== 200 || !data?.screen || !data.page) {
      const lines = (data?.errors ?? []).map((error) => `${error.file}:${error.line}: ${error.message}`);
      const heading = data?.error ?? (lines.length > 0 ? say("This page does not load") : say("There is no page to show"));
      showMessage(container, drawing, heading, lines);
      return;
    }
    const { screen, page } = data;
    drawing.registerSymbols(data.symbols ?? []);
    const document2 = container.ownerDocument;
    const kept = host2.recall?.(THEME_KEY);
    let theme = kept === "dark" || kept === "light" ? kept : screen.theme ?? "light";
    container.classList.add(`hmi-theme-${theme}`);
    const style = document2.createElement("style");
    style.textContent = drawing.tokensCss(screen.colors ?? {}, FONT_URL) + DISPLAY_CSS + MOTION_LAYER_CSS + OPERATE_CSS + FRAME_CSS + POPUP_CSS + SETTINGS_CSS + TREND_CSS + EQUIPMENT_FOCUS_CSS;
    container.append(style);
    const languages = data.languages ?? [];
    const language2 = languages.length > 1 && pinned === void 0 ? {
      current: data.language ?? languages[0].tag,
      languages,
      choose: (tag) => {
        host2.remember?.(LANGUAGE_KEY, tag);
        host2.reload();
      }
    } : void 0;
    const frame = buildFrame(container, data, page.name, translate(page.model.title), language2, host2.asset, signal);
    if (frame.topBar) {
      const toggle = document2.createElement("button");
      toggle.type = "button";
      toggle.className = "hmi-theme-switch";
      toggle.setAttribute("data-action", "theme");
      const offer = () => {
        const label = say(theme === "dark" ? "Day" : "Night");
        toggle.textContent = theme === "dark" ? "\u2600" : "\u263E";
        toggle.setAttribute("aria-label", label);
        toggle.title = label;
      };
      offer();
      toggle.addEventListener("click", () => {
        container.classList.remove(`hmi-theme-${theme}`);
        theme = theme === "dark" ? "light" : "dark";
        container.classList.add(`hmi-theme-${theme}`);
        host2.remember?.(THEME_KEY, theme);
        offer();
      });
      frame.topBar.insertBefore(toggle, frame.sessionSlot ?? null);
    }
    const surface = document2.createElement("div");
    surface.className = "hmi-surface";
    frame.main.append(surface);
    const motion = new MotionLayer(surface);
    const placedModels = Object.values(data.faceplates ?? {}).flatMap((placed) => placed ? [placed.model] : []);
    const store = new LiveValueStore({ trendSpansMs: trendSpans(page.model, placedModels) });
    const save = (name, text2) => host2.download?.(name, text2);
    const trends = new TrendActions(
      () => shown.model,
      store,
      (url2) => host2.fetchJson(url2),
      surface,
      () => host2.now(),
      {},
      save,
      (id) => expandTrend(id)
    );
    let expanded;
    const expandedModel = (objectId) => {
      const object = shown.model.object.find((candidate) => candidate.id === objectId);
      return {
        title: "",
        object: object ? [{ ...object, at: [24, 24], size: [screen.size[0] - 48, screen.size[1] - 48] }] : [],
        pipe: []
      };
    };
    function expandTrend(objectId) {
      expanded?.layer.remove();
      const layer = document2.createElement("div");
      layer.className = "hmi-trend-expanded";
      layer.setAttribute("data-part", "trend-expanded");
      const close = document2.createElement("button");
      close.type = "button";
      close.className = "hmi-trend-close";
      close.setAttribute("data-action", "close");
      close.setAttribute("aria-label", say("Close"));
      close.textContent = "\u2715";
      const inner = document2.createElement("div");
      inner.className = "hmi-trend-surface";
      layer.append(close, inner);
      container.append(layer);
      const local = new TrendActions(
        () => expandedModel(objectId),
        store,
        (url2) => host2.fetchJson(url2),
        inner,
        () => host2.now(),
        trends.views,
        save
      );
      const atIn = (event) => designPoint(inner, screen.size, event);
      const idOf = (event) => event.target?.closest?.("[data-object]")?.getAttribute("data-object") ?? void 0;
      close.addEventListener("click", () => {
        layer.remove();
        expanded = void 0;
        redraw();
      });
      inner.addEventListener("click", (event) => void local.click(event.target, idOf(event), redraw));
      inner.addEventListener("pointerdown", (event) => {
        const id = idOf(event);
        if (id && local.press(id, atIn(event), event.target)) {
          event.preventDefault();
          redraw();
        }
      });
      inner.addEventListener(
        "wheel",
        (event) => {
          const id = idOf(event);
          if (id && local.wheel(id, atIn(event), event.deltaY)) {
            event.preventDefault();
            redraw();
          }
        },
        { passive: false }
      );
      expanded = { objectId, layer, inner, trends: local, at: atIn };
      redraw();
    }
    const alarms = new AlarmView(frame.alarms ? host2.sounds : void 0);
    frame.alarms?.silence.addEventListener("click", () => {
      alarms.silence();
      redraw();
    });
    const painting = {
      mark: (priority) => drawing.renderToString(drawing.priorityMarkImage(priority, 18)),
      time: timeOfDay
    };
    const valuePaint = new ValuePaintQueue(
      (callback) => host2.schedule(callback),
      (element2, before, after) => updateNode(element2, before, after, drawing.renderToString)
    );
    let pending = false;
    let lastRenderedAt = Number.NEGATIVE_INFINITY;
    let drawn;
    const shown = { name: page.name, model: page.model, roles: data.roles ?? {}, formats: data.formats };
    let layout = { size: screen.size, fit: true };
    let laidOut;
    const render = () => {
      if (signal?.aborted) return;
      pending = false;
      lastRenderedAt = host2.now();
      const area = surface.clientWidth > 0 && surface.clientHeight > 0 ? [surface.clientWidth, surface.clientHeight] : screen.size;
      if (laidOut?.area[0] !== area[0] || laidOut.area[1] !== area[1])
        laidOut = { area, adapted: drawing.adaptPage(page.model, screen.size, area) };
      const adapted = laidOut.adapted;
      shown.model = screenCoordinates(adapted.page);
      layout = adapted;
      surface.classList.toggle("hmi-scroll", !adapted.fit);
      const drawFrame = {
        currentPage: page.name,
        size: adapted.size,
        formats: data.formats,
        ...store.frame(host2.now()),
        commands: operation.states(),
        readOnly: operation.readOnly,
        locked: operation.locked(),
        trendViews: trends.views,
        drafts: operation.drafts(),
        placed: placedFaceplates.frame(),
        asset: host2.asset ?? assetUrl,
        ...displaySpeech(),
        alarms: alarms.frame(store.isStale(host2.now()))
      };
      drawn = patchSurface(surface, drawing.drawPage(adapted.page, drawFrame), drawing.renderToString, drawn, valuePaint);
      valuePaint.flush();
      equipmentFocus.apply(surface);
      if (expanded)
        expanded.inner.innerHTML = drawing.renderToString(
          drawing.drawPage(expandedModel(expanded.objectId), { ...drawFrame, size: screen.size })
        );
      motion.sync();
      if (frame.alarms) alarms.drawTopBar(frame.alarms, store.isStale(host2.now()), painting);
    };
    const redraw = () => {
      if (signal?.aborted) return;
      popups?.redraw();
      if (!pending) {
        pending = true;
        host2.schedule(render);
      }
    };
    const equipmentFocus = new EquipmentFocus(host2, redraw);
    let socket;
    const cleanup = [];
    signal?.addEventListener(
      "abort",
      () => {
        valuePaint.dispose();
        if (socket) {
          socket.onmessage = null;
          socket.onclose = null;
          socket.close();
        }
        host2.sounds?.stop();
        cleanup.forEach((remove) => remove());
      },
      { once: true }
    );
    const listen = (target, name, handler) => {
      target?.addEventListener(name, handler);
      cleanup.push(() => target?.removeEventListener(name, handler));
    };
    const reconnect = () => {
      if (socket) {
        socket.onclose = null;
        socket.close();
      }
      connect();
    };
    const operation = new Operation(
      container,
      host2,
      shown,
      data.bundle ?? "",
      () => redraw(),
      (path) => store.frame(host2.now()).values[path],
      () => reconnect(),
      frame.sessionSlot,
      painting.mark
    );
    const follow = (target, object, source) => {
      const [kind, name] = [target.slice(0, target.indexOf(":")), target.slice(target.indexOf(":") + 1)];
      if (kind === "page") host2.navigate(`/hmi/${encodeURIComponent(name)}`);
      else if (kind === "popup") void popups?.open(name);
      else if (kind === "faceplate") void popups?.openFaceplate(`${source}/${object}`, name);
    };
    const popups = new Popups(container, host2, drawing, operation, follow);
    cleanup.push(() => popups.dispose());
    const placedFaceplates = new PlacedFaceplates(
      data.faceplates ?? {},
      operation,
      () => redraw(),
      (path) => store.frame(host2.now()).values[path],
      () => shown.model
    );
    surface.addEventListener("click", (event) => {
      const clicked = event.target;
      if (placedFaceplates.click(clicked, clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? void 0))
        return;
      const target = clicked?.closest?.("[data-object]");
      const id = target?.getAttribute("data-object");
      if (trends.click(clicked, id ?? void 0, redraw)) return;
      if (operation.alarms.click(clicked, id ?? void 0, alarms, redraw)) return;
      const option = clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? void 0;
      const goes = target?.getAttribute("data-click");
      if (goes && id) follow(goes, id, page.name);
      else if (id) operation.click(id, option);
    });
    const at = (event) => designPoint(surface, layout.size, event);
    surface.addEventListener("pointerdown", (event) => {
      operation.overlay.touched = event.pointerType === "touch";
      const id = event.target?.closest?.("[data-object]")?.getAttribute("data-object");
      if (placedFaceplates.press(event.target, at(event))) event.preventDefault();
      else if (id && trends.press(id, at(event), event.target)) {
        event.preventDefault();
        redraw();
      } else if (id && operation.press(id, at(event))) event.preventDefault();
    });
    surface.addEventListener(
      "wheel",
      (event) => {
        const id = event.target?.closest?.("[data-object]")?.getAttribute("data-object");
        if (id && trends.wheel(id, at(event), event.deltaY)) {
          event.preventDefault();
          redraw();
        }
      },
      { passive: false }
    );
    const view = document2.defaultView ?? void 0;
    listen(view, "resize", () => redraw());
    const pointerTarget = view ?? document2;
    listen(pointerTarget, "pointermove", (event) => {
      const over = event.target?.closest?.("[data-object]")?.getAttribute("data-object") ?? void 0;
      const open = expanded;
      if (open) {
        const inside = open.inner.contains(event.target);
        if (open.trends.drag(open.at(event)) || open.trends.hover(inside ? over : void 0, open.at(event)))
          redraw();
      } else if (trends.drag(at(event)) || trends.hover(over, at(event))) redraw();
      operation.drag(at(event));
      placedFaceplates.drag(at(event));
    });
    listen(pointerTarget, "pointerup", (event) => {
      if (expanded?.trends.release(expanded.at(event)) || trends.release(at(event))) redraw();
      operation.release(at(event));
      placedFaceplates.release(at(event));
    });
    await operation.start();
    if (signal?.aborted) return;
    await loadRecords(host2, page.model, alarms);
    if (signal?.aborted) return;
    await loadHistory((url2) => host2.fetchJson(url2), trendSpans(page.model, placedModels), store);
    if (signal?.aborted) return;
    const connect = () => {
      if (signal?.aborted) return;
      const opened = host2.openSocket(socketUrl(host2.location, `page=${encodeURIComponent(page.name)}`));
      socket = opened;
      opened.onmessage = (event) => {
        if (signal?.aborted) return;
        const message = parseMessage(event.data);
        if (!message) return;
        if ("session" in message || "command" in message || "bundle" in message) {
          operation.receive(message);
          return;
        }
        if ("alarms" in message || "events" in message) {
          alarms.receive(message);
          redraw();
          return;
        }
        if ("simulation" in message) {
          showSimulation(frame.simulation, message.simulation);
          return;
        }
        store.receive(message, host2.now());
        redraw();
      };
      opened.onclose = () => {
        if (signal?.aborted) return;
        store.disconnected(host2.now());
        redraw();
        host2.later(RECONNECT_MS2, connect);
      };
    };
    const tick = () => {
      if (signal?.aborted) return;
      const now = host2.now();
      frame.tick(now);
      if (store.isStale(now) || now - lastRenderedAt >= REDRAW_MS) redraw();
      host2.later(REDRAW_MS, tick);
    };
    connect();
    frame.tick(host2.now());
    host2.later(REDRAW_MS, tick);
    redraw();
  }
  function pageNameOf(pathname) {
    const name = pathname.replace(/^\/hmi\/?/, "").replace(/\/$/, "");
    return name === "" ? void 0 : decodeURIComponent(name);
  }
  function showMessage(container, drawing, heading, lines) {
    const document2 = container.ownerDocument;
    container.classList.add("hmi-theme-dark");
    const style = document2.createElement("style");
    style.textContent = drawing.tokensCss({}, FONT_URL) + DISPLAY_CSS;
    const box = document2.createElement("div");
    box.className = "hmi-message";
    const section = document2.createElement("section");
    const title = document2.createElement("h1");
    title.textContent = heading;
    section.append(title);
    for (const line of lines) {
      const paragraph = document2.createElement("p");
      paragraph.textContent = line;
      section.append(paragraph);
    }
    box.append(section);
    container.append(style, box);
  }

  // src/hmiBuilder/display/webview/runtimeDisplayMain.ts
  var host = {
    location: window.location,
    // A browser's choices, such as its language (§14.5); storage can be off, then nothing is kept.
    remember(key, value) {
      try {
        window.localStorage.setItem(key, value);
      } catch {
      }
    },
    recall(key) {
      try {
        return window.localStorage.getItem(key) ?? void 0;
      } catch {
        return void 0;
      }
    },
    async fetchJson(url) {
      const response = await fetch(url, { cache: "no-store", headers: { Accept: "application/json" } });
      const body = await response.json().catch(() => null);
      return { status: response.status, body };
    },
    async postJson(url, body) {
      const response = await fetch(url, {
        method: "POST",
        cache: "no-store",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(body)
      });
      const answer = await response.json().catch(() => null);
      return { status: response.status, body: answer };
    },
    openSocket(url) {
      const socket = new WebSocket(url);
      const stream = { onmessage: null, onclose: null, close: () => socket.close() };
      socket.onmessage = (event) => stream.onmessage?.({ data: event.data });
      socket.onclose = () => stream.onclose?.();
      return stream;
    },
    now: () => Date.now(),
    schedule: (callback) => {
      window.requestAnimationFrame(() => callback());
    },
    later: (milliseconds, callback) => {
      window.setTimeout(callback, milliseconds);
    },
    reload: () => window.location.reload(),
    navigate: (url) => window.location.assign(url),
    sounds: browserSounds(window),
    download(name, text2) {
      const url = URL.createObjectURL(new Blob([text2], { type: "text/csv" }));
      const link2 = document.createElement("a");
      link2.href = url;
      link2.download = name;
      document.body.append(link2);
      link2.click();
      link2.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 1e3);
    }
  };
  function start() {
    const container = document.getElementById("hmi");
    if (container) void startDisplay(container, host, TrustHmiDrawing);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
