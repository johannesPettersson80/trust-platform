// Generated from editors/vscode/src/hmiBuilder/drawing by `npm run build:hmi-drawing`; do not edit.
"use strict";
var TrustHmiDrawing = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // src/hmiBuilder/drawing/runtimeBundle.ts
  var runtimeBundle_exports = {};
  __export(runtimeBundle_exports, {
    TOKEN_THEMES: () => TOKEN_THEMES,
    adaptPage: () => adaptPage,
    designValues: () => designValues,
    drawPage: () => drawPage,
    priorityMarkImage: () => priorityMarkImage,
    registerSymbols: () => registerSymbols,
    renderToString: () => renderToString,
    tokensCss: () => tokensCss
  });

  // src/hmiBuilder/drawing/cuts.ts
  var collecting;
  function drawingObject(id, draw) {
    const now = collecting;
    if (!now) return draw();
    const before = now.object;
    now.object = id;
    try {
      return draw();
    } finally {
      now.object = before;
    }
  }
  function heard(text2) {
    collecting?.said.add(text2);
    return text2;
  }
  function noteCut(text2) {
    const now = collecting;
    if (now?.object === void 0 || !now.said.has(text2)) return;
    if (!now.cuts.some((cut) => cut.object === now.object && cut.text === text2))
      now.cuts.push({ object: now.object, text: text2 });
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
  function withSpeech(frame, draw) {
    const before = current;
    current = { texts: frame.texts ?? base.texts, language: frame.language ?? base.language, longest: frame.longest };
    try {
      return draw();
    } finally {
      current = before;
    }
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
  function language() {
    return current.language;
  }
  function fill(text2, values) {
    return text2.replace(/\{([A-Za-z]+)\}/g, (whole, name) => name in values ? String(values[name]) : whole);
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
    const match = DURATION.exec(literal);
    if (!match || literal === "T#" || literal === "T#-") return void 0;
    const [, minus, days, hours, minutes, seconds, millis, micros, nanos] = match;
    if (decimals !== void 0) {
      const scale = 10 ** decimals;
      const total = Math.round(
        (Number(days ?? 0) * 86400 + Number(hours ?? 0) * 3600 + Number(minutes ?? 0) * 60 + Number(seconds ?? 0) + Number(millis ?? 0) / 1e3 + Number(micros ?? 0) / 1e6 + Number(nanos ?? 0) / 1e9) * scale
      ) / scale;
      const whole = Math.floor(total);
      const rounded2 = [];
      if (whole >= 86400) rounded2.push(`${Math.floor(whole / 86400)} d`);
      if (whole % 86400 >= 3600) rounded2.push(`${Math.floor(whole % 86400 / 3600)} h`);
      if (whole % 3600 >= 60) rounded2.push(`${Math.floor(whole % 3600 / 60)} min`);
      const rest = total - Math.floor(total / 60) * 60;
      if (rest > 0 || rounded2.length === 0) rounded2.push(`${localNumber(rest, decimals, decimals)} s`);
      return `${minus ?? ""}${rounded2.join(" ")}`;
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
  function fitWidth(content, width, size, weight = 400) {
    if (textWidth(content, size, weight) <= width) return content;
    noteCut(content);
    const letters = Array.from(content);
    while (letters.length > 0 && textWidth(`${letters.join("")}\u2026`, size, weight) > width) letters.pop();
    return `${letters.join("").trimEnd()}\u2026`;
  }
  function fitText(content, width, charWidth) {
    const fits = Math.max(1, Math.floor(width / charWidth));
    return content.length > fits ? `${content.slice(0, Math.max(0, fits - 1))}\u2026` : content;
  }
  function timeOfDay(ms) {
    const time = new Date(ms);
    const two = (value) => String(value).padStart(2, "0");
    return `${two(time.getHours())}:${two(time.getMinutes())}:${two(time.getSeconds())}`;
  }
  function formatAge(milliseconds) {
    const seconds = Math.max(0, Math.round(milliseconds / 1e3));
    if (seconds < 60) return say("{seconds} s", { seconds });
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return say("{minutes} min", { minutes });
    return say("{hours} h", { hours: Math.floor(minutes / 60) });
  }

  // src/hmiBuilder/drawing/svg.ts
  function el(tag, attrs = {}, children = []) {
    const clean = {};
    for (const [name, value] of Object.entries(attrs)) {
      if (value !== void 0) clean[name] = String(value);
    }
    return { tag, attrs: clean, children };
  }
  function text(content, attrs = {}) {
    return { ...el("text", attrs), text: content };
  }
  function tspan(content, attrs = {}) {
    return { ...el("tspan", attrs), text: content };
  }
  function run(attrs, spans) {
    return el("text", attrs, spans);
  }
  var ESCAPES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };
  function escape(value) {
    return value.replace(/[&<>"]/g, (character) => ESCAPES[character]);
  }
  function renderToString(node) {
    const attrs = Object.entries(node.attrs).map(([name, value]) => ` ${name}="${escape(value)}"`).join("");
    const inner = (node.text !== void 0 ? escape(node.text) : "") + node.children.map(renderToString).join("");
    return `<${node.tag}${attrs}>${inner}</${node.tag}>`;
  }

  // src/hmiBuilder/drawing/parts.ts
  var UNKNOWN_HATCH = "hmi-unknown-hatch";
  var GLOW = "url(#hmi-glow)";
  var ACCENT_ACROSS = "url(#hmi-accent-x)";
  var ACCENT_UP = "url(#hmi-accent-y)";
  var SECOND_UP = "url(#hmi-second-y)";
  function token(name) {
    return `var(--hmi-${name})`;
  }
  function gradient(id, from, to, stops) {
    return el(
      "linearGradient",
      { id, x1: from[0], y1: from[1], x2: to[0], y2: to[1] },
      stops.map((name, index) => el("stop", { offset: index / (stops.length - 1), "stop-color": token(name) }))
    );
  }
  function definitions() {
    return el("defs", {}, [
      el(
        "pattern",
        { id: UNKNOWN_HATCH, patternUnits: "userSpaceOnUse", width: 8, height: 8, patternTransform: "rotate(45)" },
        [el("line", { x1: 0, y1: 0, x2: 0, y2: 8, stroke: token("unknown"), "stroke-width": 2, "stroke-opacity": 0.55 })]
      ),
      gradient("hmi-accent-x", [0, 0], [1, 0], ["accent", "accent-2"]),
      gradient("hmi-accent-y", [0, 1], [0, 0], ["accent-2", "accent"]),
      gradient("hmi-second-y", [0, 1], [0, 0], ["accent-2", "accent-3"]),
      el("radialGradient", { id: "hmi-backdrop", cx: 0.12, cy: 0, r: 0.85 }, [
        el("stop", { offset: 0, "stop-color": token("accent-2"), "stop-opacity": 0.16 }),
        el("stop", { offset: 1, "stop-color": token("accent-2"), "stop-opacity": 0 })
      ]),
      el("filter", { id: "hmi-glow", x: "-50%", y: "-50%", width: "200%", height: "200%" }, [
        el("feGaussianBlur", { in: "SourceGraphic", stdDeviation: 3.5, result: "blur" }),
        el("feMerge", {}, [el("feMergeNode", { in: "blur" }), el("feMergeNode", { in: "SourceGraphic" })])
      ])
    ]);
  }
  function backdrop(width, height) {
    return [
      el("rect", { "data-part": "page", x: 0, y: 0, width, height, fill: token("page") }),
      el("rect", { x: 0, y: 0, width, height, fill: "url(#hmi-backdrop)" })
    ];
  }
  function fontSize(height, share2 = 0.42, maximum = 44) {
    return Math.min(maximum, Math.max(14, Math.round(height * share2)));
  }
  function card(width, height, live, radius = 14) {
    const stale = live?.staleMs !== void 0;
    return el("rect", {
      "data-part": "card",
      x: 0.5,
      y: 0.5,
      width: Math.max(0, width - 1),
      height: Math.max(0, height - 1),
      rx: radius,
      fill: token("surface"),
      stroke: stale ? token("stale") : token("border")
    });
  }
  function hatchBox(box, radius = 3) {
    return el("rect", { ...box, rx: radius, fill: `url(#${UNKNOWN_HATCH})` });
  }
  function hatch(width, height, radius = 13) {
    return hatchBox({ x: 1.5, y: 1.5, width: Math.max(0, width - 3), height: Math.max(0, height - 3) }, radius);
  }
  function estimatedMark(at) {
    return text(say("est."), { ...at, fill: token("muted"), "font-size": 14, "font-style": "italic" });
  }
  function commandedMark(object, live, at) {
    return text(`\u2192 ${formatValue(live.target, object)}`, {
      ...at,
      fill: token("pending"),
      "font-size": 14,
      "font-weight": 600
    });
  }
  function staleMark(live, at) {
    return text(formatAge(live.staleMs ?? 0), { ...at, fill: token("stale"), "font-size": 14, "font-weight": 600 });
  }
  function chip(label, colour, right, top, italic) {
    const width = Math.round(textWidth(label, 14, 600) + 16);
    return {
      width,
      node: el("g", { "data-part": "chip" }, [
        el("rect", { x: right - width, y: top, width, height: 22, rx: 11, fill: token(colour), "fill-opacity": 0.14 }),
        text(label, {
          x: right - width / 2,
          y: top + 15.5,
          fill: token(colour),
          "font-size": 14,
          "font-weight": 600,
          "font-style": italic ? "italic" : void 0,
          "text-anchor": "middle"
        })
      ])
    };
  }
  function stateChips(object, live, width, top = 12) {
    return stateChipRow(object, live, width, top).nodes;
  }
  function stateChipRow(object, live, width, top = 12) {
    if (!live) return { nodes: [], left: width };
    const chips = [];
    if (live.staleMs !== void 0) chips.push([formatAge(live.staleMs), "stale", false]);
    if (live.state === "commanded") chips.push([`\u2192 ${formatValue(live.target, object)}`, "pending", false]);
    if (live.state === "estimated") chips.push([say("est."), "muted", true]);
    const nodes = [];
    let right = width - 12;
    for (const [label, colour, italic] of chips) {
      const placed3 = chip(label, colour, right, top, italic);
      nodes.push(placed3.node);
      right -= placed3.width + 6;
    }
    return { nodes, left: nodes.length > 0 ? right + 6 : width };
  }
  function stateMarks(object, live, width, height, staleY = 16) {
    if (!live) return [];
    const marks = [];
    if (live.state === "estimated") marks.push(estimatedMark({ x: 6, y: height - 6 }));
    if (live.state === "commanded")
      marks.push(commandedMark(object, live, { x: width - 6, y: height - 6, "text-anchor": "end" }));
    if (live.staleMs !== void 0) marks.push(staleMark(live, { x: width - 6, y: staleY, "text-anchor": "end" }));
    return marks;
  }
  function stateAttributes(live) {
    return {
      "data-state": live?.state,
      "data-stale": live?.staleMs !== void 0 ? "true" : void 0
    };
  }

  // src/hmiBuilder/drawing/symbols/style.ts
  var LINE = {
    stroke: token("muted"),
    "stroke-width": 2,
    "stroke-linejoin": "round",
    "stroke-linecap": "round",
    "vector-effect": "non-scaling-stroke"
  };
  var BODY = token("raised");
  var FILLED = token("muted");
  var MOVING = token("text");
  var STILL = token("muted");
  var LIQUID = ACCENT_UP;
  var HATCH = `url(#${UNKNOWN_HATCH})`;
  function round(value) {
    return Math.round(value * 100) / 100;
  }

  // src/hmiBuilder/drawing/symbols/custom.ts
  function customSymbol(model) {
    const [x, y, width, height] = model.viewBox;
    return {
      size: [width, height],
      ports: Object.fromEntries(
        model.ports.map((port) => [
          port.name,
          { at: [port.at[0] - x, port.at[1] - y], direction: port.direction }
        ])
      ),
      properties: Object.keys(model.properties),
      deciding: [],
      states: [],
      sample: "",
      read: (_value, _frame, faulted) => ({
        state: faulted ? "fault" : void 0,
        word: faulted ? say("Fault") : "",
        fault: faulted,
        view: { unknown: false }
      }),
      rules: model.parts,
      // `currentColor` in the drawing is the line colour of the built-in symbols, so a drawing that
      // uses it follows the dark and light themes and the project's colours.
      draw: (view) => [
        el("g", { transform: x === 0 && y === 0 ? void 0 : `translate(${-x} ${-y})`, color: token("muted") }, [
          drawn(model.svg, view.parts ?? {})
        ])
      ]
    };
  }
  function drawn(node, parts, inheritedFill) {
    const fill2 = node.attrs.fill ?? inheritedFill;
    const view = node.attrs["data-part"] !== void 0 ? parts[node.attrs["data-part"]] : void 0;
    const children = node.children.map((child) => drawn(child, parts, fill2));
    if (!view) return { ...node, children };
    const attrs = { ...node.attrs };
    const classes = [];
    const styles = [];
    if (view.spin) {
      classes.push("hmi-spin");
      styles.push(`animation-delay: -${view.spin.delayMs}ms`);
    }
    if (view.stroke) {
      classes.push("hmi-stroke");
      styles.push(`animation-delay: -${view.stroke.delayMs}ms`);
    }
    if (view.moved) styles.push(`transform: translate(${round(view.moved[0])}px, ${round(view.moved[1])}px)`);
    if (view.hidden) attrs.display = "none";
    if (view.blink) classes.push("hmi-blink");
    if (view.angle !== void 0)
      styles.push(`transform-box: fill-box; transform-origin: center; transform: rotate(${round(view.angle)}deg)`);
    if (view.level !== void 0)
      styles.push(`transform-box: fill-box; transform-origin: 50% 100%; transform: scaleY(${round(view.level)})`);
    const paint2 = view.alarm ? token("alarm") : view.active ? MOVING : void 0;
    if (paint2 && fill2 === "none") attrs.stroke = paint2;
    else if (paint2) attrs.fill = paint2;
    if (view.active && !view.alarm) attrs.filter = GLOW;
    if (classes.length > 0) attrs.class = classes.join(" ");
    if (styles.length > 0) attrs.style = styles.join("; ");
    return view.text !== void 0 ? { ...node, attrs, children: [], text: view.text } : { ...node, attrs, children };
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
    const range2 = object.type === "bar" || object.type === "gauge" ? equipment?.range : void 0;
    const min = object.min ?? range2?.[0], max = object.max ?? range2?.[1];
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
  function liveValueOf(object, property, frame) {
    const binding = object.bind[property];
    if (!binding) return void 0;
    const value = frame.values[binding.path] ?? UNKNOWN;
    if (!binding.quality) return value;
    return withQuality(value, frame.values[binding.quality]);
  }
  function withQuality(value, quality) {
    value = value ?? UNKNOWN;
    if (!quality || quality.state === "unknown" || quality.staleMs !== void 0) return { ...value, state: "unknown" };
    const status = String(quality.value).toLowerCase();
    if (status === "true" || status === "good") return value;
    if (status === "uncertain") return { ...value, state: "estimated" };
    return { ...value, state: "unknown" };
  }
  function isOn(value) {
    if (typeof value === "boolean") return value;
    if (typeof value === "number") return value !== 0;
    if (typeof value === "string") return !["", "0", "false", "off"].includes(value.trim().toLowerCase());
    return false;
  }

  // src/hmiBuilder/drawing/symbols/motion.ts
  var SPIN_PERIOD_MS = 2e3;
  var FLOW_PERIOD_MS = 800;
  function observed(value) {
    return value !== void 0 && value.staleMs === void 0 && (value.state === "confirmed" || value.state === "estimated");
  }
  function turnDelayMs(nowMs) {
    return Math.floor(nowMs ?? 0) % SPIN_PERIOD_MS;
  }

  // src/hmiBuilder/drawing/symbols/reading.ts
  function on(value) {
    return value !== void 0 && value.state !== "unknown" && isOn(value.value);
  }
  function turning(value, frame) {
    return on(value) && observed(value) ? { delayMs: turnDelayMs(frame.nowMs) } : void 0;
  }
  var readRunning = (value, frame, faulted) => {
    const running = value("running");
    const runs = on(running);
    const word = running ? say(runs ? "Running" : "Stopped") : "";
    return {
      state: faulted ? "fault" : running ? runs ? "running" : "stopped" : void 0,
      word: faulted ? say("Fault") : word,
      fault: faulted,
      view: { unknown: false, turning: turning(running, frame) }
    };
  };
  function shownNumber(value, object) {
    const number2 = Number(value?.value);
    if (value === void 0 || value.value === null || !Number.isFinite(number2)) return void 0;
    const text2 = formatValue(number2, { decimals: object.decimals });
    return object.unit ? `${text2} ${object.unit}` : text2;
  }
  function isShare(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value) && Object.keys(value).length === 1 && typeof value.share === "number";
  }
  function resolveState(value, object) {
    if (!isShare(value)) return value;
    const [min, max] = [object.min ?? 0, object.max ?? 100];
    return Number((min + (max - min) * value.share).toFixed(6));
  }
  function runningStates(extra = () => ({})) {
    const state5 = (id, name, running, flags = {}) => {
      const set = Object.keys(flags);
      return {
        id,
        name,
        values: { running, fault: false, manual: false, "out-of-service": false, ...extra(running), ...flags },
        needs: set.length > 0 ? set : ["running"]
      };
    };
    return [
      state5("running", "Running", true),
      state5("stopped", "Stopped", false),
      state5("fault", "Fault", false, { fault: true }),
      state5("manual", "Manual", true, { manual: true }),
      state5("out-of-service", "Out of service", false, { "out-of-service": true })
    ];
  }
  function percentWord(value) {
    const number2 = Number(value?.value);
    if (value === void 0 || value.value === null || !Number.isFinite(number2)) return void 0;
    return say("{value} %", { value: formatValue(Math.round(number2), {}) });
  }
  function readRunningWith(property, percent) {
    return (value, frame, faulted, object) => {
      const reading = readRunning(value, frame, faulted, object);
      if (reading.state !== "running") return reading;
      const shown = percent ? percentWord(value(property)) : shownBoundNumber(value, property, frame, object);
      return shown ? { ...reading, word: shown } : reading;
    };
  }
  function readNumbers(...properties) {
    return (value, frame, faulted, object) => {
      const shown = properties.map((property) => shownBoundNumber(value, property, frame, object)).filter((text2) => text2 !== void 0);
      return {
        state: faulted ? "fault" : void 0,
        word: faulted ? say("Fault") : shown.join(" \xB7 "),
        fault: faulted,
        view: { unknown: false }
      };
    };
  }
  function shownBoundNumber(value, property, frame, object) {
    return shownNumber(value(property), withValueFormat(object, frame, property));
  }

  // src/hmiBuilder/drawing/symbols/valve.ts
  var AXIS = 44;
  var MIDDLE = 32;
  var BODY_PATH = `M6 32 L${MIDDLE} ${AXIS} L6 56 Z M58 32 L${MIDDLE} ${AXIS} L58 56 Z`;
  var STUBS_PATH = `M0 ${AXIS} H6 M58 ${AXIS} H64`;
  function state(id, name, open, closed, flags = {}) {
    const set = Object.keys(flags);
    const needs = set.length > 0 ? set : open || closed ? ["open|closed"] : ["open", "closed"];
    return { id, name, values: { open, closed, fault: false, manual: false, "out-of-service": false, ...flags }, needs };
  }
  var VALVE = {
    properties: ["open", "closed", "fault", "manual", "out-of-service"],
    deciding: ["fault", "open", "closed"],
    sample: "open",
    read: (value, _frame, faulted) => readFeedback(value("open"), value("closed"), faulted),
    states: [
      state("open", "Open", true, false),
      state("closed", "Closed", false, true),
      state("moving", "Moving", false, false),
      state("fault", "Fault", false, true, { fault: true }),
      state("manual", "Manual", true, false, { manual: true }),
      state("out-of-service", "Out of service", false, true, { "out-of-service": true })
    ],
    draw: (view) => {
      const angle = 90 * (1 - (view.opening ?? 0));
      return [
        el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
        el("path", { "data-part": "stem", d: `M${MIDDLE} 36 V22`, fill: "none", ...LINE }),
        el("rect", { "data-part": "actuator", x: 21, y: 10, width: 22, height: 12, rx: 3, fill: BODY, ...LINE }),
        el("path", {
          "data-part": "body",
          "data-filled": String(view.filled === true),
          d: BODY_PATH,
          fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
          ...LINE
        }),
        el("circle", { "data-part": "ball", cx: MIDDLE, cy: AXIS, r: 8, fill: BODY, ...LINE }),
        ...view.unknown ? [] : [
          el("rect", {
            "data-part": "bore",
            "data-angle": angle,
            x: MIDDLE - 7,
            y: AXIS - 2,
            width: 14,
            height: 4,
            rx: 2,
            fill: MOVING,
            transform: angle === 0 ? void 0 : `rotate(${angle} ${MIDDLE} ${AXIS})`
          })
        ]
      ];
    }
  };
  function readFeedback(open, closed, faulted, ends = { open: "Open", closed: "Closed" }) {
    let stands;
    if (open && closed) stands = on(open) ? on(closed) ? "both" : "open" : on(closed) ? "closed" : "moving";
    else if (open) stands = on(open) ? "open" : "closed";
    else if (closed) stands = on(closed) ? "closed" : "open";
    const fault = faulted || stands === "both";
    const words = { open: ends.open, closed: ends.closed, moving: "Moving", both: "Fault" };
    const opening = stands === "open" ? 1 : stands === "closed" ? 0 : stands ? 0.5 : void 0;
    return {
      state: fault ? "fault" : stands,
      word: fault ? say("Fault") : stands ? say(words[stands]) : "",
      fault,
      view: { unknown: false, opening, filled: stands === "closed" }
    };
  }

  // src/hmiBuilder/drawing/symbols/controlValve.ts
  var TRAVEL = 13;
  var ACTUATOR = 20;
  function state2(id, name, position, flags = {}, demand = position) {
    const set = Object.keys(flags);
    const needs = set.length > 0 ? set : demand !== position ? ["position", "demand"] : ["position"];
    return {
      id,
      name,
      values: { position, demand, fault: false, manual: false, "out-of-service": false, ...flags },
      needs
    };
  }
  function plugTop(lift) {
    return AXIS - 8 - TRAVEL * Math.min(1, Math.max(0, lift));
  }
  var CONTROL_VALVE = {
    properties: ["position", "fault", "manual", "out-of-service", "demand"],
    deciding: ["fault", "position"],
    sample: "partly-open",
    read: (value, _frame, faulted) => readPosition(value("position"), value("demand"), faulted),
    rules: [{ part: "demand", travel: "demand" }],
    states: [
      state2("closed", "Closed", 0),
      state2("partly-open", "60 %", 60),
      state2("moving", "Moving", 40, {}, 70),
      state2("open", "Open", 100),
      state2("fault", "Fault", 60, { fault: true }),
      state2("manual", "Manual", 60, { manual: true }),
      state2("out-of-service", "Out of service", 0, { "out-of-service": true })
    ],
    draw: (view) => {
      const lift = Math.min(1, Math.max(0, view.opening ?? 0));
      const top = plugTop(lift);
      const demand = view.unknown ? void 0 : view.parts?.demand?.travel;
      return [
        // The positioner first, so the stem and the plug lie over its link.
        ...view.variant === "positioner" ? [
          el("g", { "data-part": "positioner" }, [
            el("path", { d: `M${MIDDLE} 27 H42`, fill: "none", ...LINE }),
            el("rect", { x: 42, y: 22, width: 14, height: 10, rx: 2, fill: BODY, ...LINE })
          ])
        ] : [],
        el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
        el("path", {
          "data-part": "body",
          "data-filled": String(view.filled === true),
          d: BODY_PATH,
          fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
          ...LINE
        }),
        el("path", {
          "data-part": "stem",
          d: `M${MIDDLE} ${view.unknown ? AXIS - 8 : round(top)} V${ACTUATOR}`,
          fill: "none",
          ...LINE
        }),
        el("path", { "data-part": "actuator", d: `M15 ${ACTUATOR} A17 12 0 0 1 49 ${ACTUATOR} Z`, fill: BODY, ...LINE }),
        ...view.unknown ? [] : [
          el("path", {
            "data-part": "plug",
            "data-lift": round(lift),
            d: `M${MIDDLE - 4} ${round(top)} H${MIDDLE + 4} V${round(top + 3)} Q${MIDDLE + 4} ${round(top + 8)} ${MIDDLE} ${round(top + 8)} Q${MIDDLE - 4} ${round(top + 8)} ${MIDDLE - 4} ${round(top + 3)} Z`,
            fill: MOVING
          })
        ],
        ...demand !== void 0 ? [
          el("path", {
            "data-part": "demand",
            "data-travel": round(demand),
            d: `M22 ${round(plugTop(demand) - 3)} L26.5 ${round(plugTop(demand))} L22 ${round(plugTop(demand) + 3)} Z`,
            fill: token("muted")
          })
        ] : []
      ];
    }
  };
  function readPosition(position, demand, faulted) {
    const travel = Number(position?.value);
    if (position === void 0 || !Number.isFinite(travel)) {
      return {
        state: faulted ? "fault" : void 0,
        word: faulted ? say("Fault") : "",
        fault: faulted,
        view: { unknown: false }
      };
    }
    const percent = Math.min(100, Math.max(0, travel));
    const at = say("{value} %", { value: formatValue(Math.round(percent), {}) });
    const wanted = Number(demand?.value);
    const sent = demand !== void 0 && demand.value !== null && Number.isFinite(wanted) && Math.round(wanted) !== Math.round(percent) ? percentWord(demand) : void 0;
    return {
      state: faulted ? "fault" : percent <= 0 ? "closed" : "open",
      word: faulted ? say("Fault") : sent ? `${at} \u2192 ${sent}` : at,
      fault: faulted,
      view: { unknown: false, opening: percent / 100, filled: percent <= 0 }
    };
  }

  // src/hmiBuilder/drawing/symbols/device.ts
  var DEVICE = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    states: runningStates(),
    sample: "running",
    read: readRunning,
    rules: [{ part: "lamp", active: "running" }],
    draw: (view) => [
      el("rect", {
        "data-part": "body",
        x: 8,
        y: 12,
        width: 48,
        height: 40,
        rx: 8,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("path", {
        "data-part": "panel",
        d: "M18 26 H36 M18 33 H32 M18 40 H36",
        fill: "none",
        ...LINE,
        "stroke-linecap": "round"
      }),
      el("circle", {
        "data-part": "lamp",
        cx: 45,
        cy: 23,
        r: 4,
        fill: view.unknown ? "none" : view.parts?.lamp?.active ? MOVING : STILL,
        ...LINE
      })
    ]
  };

  // src/hmiBuilder/drawing/symbols/dosingPump.ts
  var DOSING_PUMP = {
    properties: ["running", "fault", "manual", "out-of-service", "rate"],
    deciding: ["fault", "running"],
    states: runningStates((running) => ({ rate: running ? { share: 0.6 } : 0 })),
    sample: "running",
    read: readRunningWith("rate", false),
    rules: [{ part: "piston", stroke: "running" }],
    draw: (view) => {
      const stroke = view.parts?.piston?.stroke;
      const paint2 = stroke ? MOVING : STILL;
      return [
        el("path", { "data-part": "suction", d: "M14 45 V64", fill: "none", ...LINE }),
        el("path", { "data-part": "discharge", d: "M14 0 V21", fill: "none", ...LINE }),
        // The way the liquid goes: in from below, out above.
        el("path", {
          "data-part": "flow",
          d: "M10.5 58 L14 53 L17.5 58 Z M10.5 11 L14 6 L17.5 11 Z",
          fill: token("muted")
        }),
        el("rect", {
          "data-part": "cylinder",
          x: 6,
          y: 22,
          width: 34,
          height: 22,
          rx: 4,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        }),
        ...view.unknown ? [] : [
          el(
            "g",
            {
              "data-part": "piston",
              class: stroke ? "hmi-stroke" : void 0,
              style: stroke ? `animation-delay: -${stroke.delayMs}ms` : void 0
            },
            [
              el("rect", { x: 22, y: 25, width: 4, height: 16, rx: 1, fill: paint2 }),
              el("path", {
                d: "M26 33 H48",
                fill: "none",
                stroke: paint2,
                "stroke-width": 2.5,
                "stroke-linecap": "round",
                "vector-effect": "non-scaling-stroke"
              })
            ]
          )
        ],
        el("rect", { "data-part": "drive", x: 46, y: 25, width: 14, height: 16, rx: 3, fill: BODY, ...LINE })
      ];
    }
  };

  // src/hmiBuilder/drawing/symbols/plant.ts
  function rotor(x, y, radius, view, children, part = "rotor") {
    return el(
      "g",
      {
        "data-part": part,
        class: view.turning ? "hmi-spin" : void 0,
        style: view.turning ? `animation-delay: -${view.turning.delayMs}ms` : void 0
      },
      [el("circle", { cx: x, cy: y, r: radius, fill: "none", stroke: "none" }), ...children]
    );
  }
  var paint = (view) => view.turning ? MOVING : STILL;
  var VARIABLE = el("path", {
    "data-part": "variable",
    d: "M3 61 L59 5 M59 5 L51.3 7.1 M59 5 L56.9 12.7",
    fill: "none",
    ...LINE
  });
  var MACHINE = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    sample: "running",
    read: readRunning,
    states: runningStates()
  };
  var FAN = {
    ...MACHINE,
    properties: [...MACHINE.properties, "speed"],
    read: readRunningWith("speed", true),
    states: runningStates((running) => ({ speed: running ? 62 : 0 })),
    draw: (view) => [
      ...view.variant === "variable-speed" ? [VARIABLE] : [],
      el("circle", { "data-part": "housing", cx: 32, cy: 32, r: 26, fill: view.unknown ? HATCH : BODY, ...LINE }),
      ...view.unknown ? [] : [
        rotor(32, 32, 20, view, [
          ...[0, 120, 240].map(
            (angle) => el("ellipse", {
              cx: 32,
              cy: 21,
              rx: 4.5,
              ry: 10,
              fill: paint(view),
              transform: `rotate(${angle} 32 32)`
            })
          ),
          el("circle", { cx: 32, cy: 32, r: 3.5, fill: BODY, ...LINE })
        ])
      ]
    ]
  };
  var BLOWER = {
    ...MACHINE,
    draw: (view) => [
      el("path", { d: "M0 32 H6 M58 32 H64", fill: "none", ...LINE }),
      el("circle", { "data-part": "casing", cx: 32, cy: 32, r: 26, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("path", { d: "M10 18 L54 26 M10 46 L54 38", fill: "none", ...LINE }),
      ...view.unknown ? [] : [rotor(32, 32, 20, view, [el("circle", { cx: 32, cy: 14, r: 3, fill: paint(view) })])]
    ]
  };
  var AGITATOR = {
    ...MACHINE,
    draw: (view) => [
      el("rect", {
        "data-part": "motor",
        x: 20,
        y: 2,
        width: 24,
        height: 15,
        rx: 3,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      text("M", { x: 32, y: 14, fill: token("muted"), "font-size": 11, "font-weight": 600, "text-anchor": "middle" }),
      el("path", {
        "data-part": "shaft",
        d: "M32 17 V41",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 3,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke"
      }),
      ...view.unknown ? [] : [
        rotor(32, 50, 12, view, [
          ...[0, 90].map(
            (angle) => el("ellipse", {
              cx: 32,
              cy: 50,
              rx: 12,
              ry: 3.5,
              fill: paint(view),
              transform: `rotate(${angle} 32 50)`
            })
          ),
          el("circle", { cx: 32, cy: 50, r: 3, fill: BODY, ...LINE })
        ])
      ]
    ]
  };
  function roller(x, view, part) {
    return rotor(
      x,
      20,
      9,
      view,
      [
        el("path", {
          d: `M${x - 6} 20 H${x + 6} M${x} 14 V26`,
          fill: "none",
          stroke: paint(view),
          "stroke-width": 2,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        })
      ],
      part
    );
  }
  var CONVEYOR = {
    ...MACHINE,
    draw: (view) => [
      el("path", { "data-part": "legs", d: "M14 29 L10 39 M82 29 L86 39", fill: "none", ...LINE }),
      el("rect", {
        "data-part": "belt",
        x: 4,
        y: 10,
        width: 88,
        height: 20,
        rx: 10,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("circle", { cx: 14, cy: 20, r: 9, fill: BODY, ...LINE }),
      el("circle", { cx: 82, cy: 20, r: 9, fill: BODY, ...LINE }),
      ...view.unknown ? [] : [roller(14, view, "rotor"), roller(82, view, "rotor-2")]
    ]
  };
  function standing(id, name, open, closed, extra = {}, flags = {}) {
    const set = Object.keys(flags);
    return {
      id,
      name,
      values: {
        open,
        closed,
        fault: false,
        manual: false,
        "out-of-service": false,
        ...extra,
        ...flags
      },
      needs: set.length > 0 ? set : ["open|closed"]
    };
  }
  var STANDING = (extra = () => ({})) => [
    standing("open", "Open", true, false, extra(true)),
    standing("closed", "Closed", false, true, extra(false)),
    standing("moving", "Moving", false, false, extra(false)),
    standing("fault", "Fault", false, true, extra(false), { fault: true }),
    standing("manual", "Manual", true, false, extra(true), { manual: true }),
    standing("out-of-service", "Out of service", false, true, extra(false), { "out-of-service": true })
  ];
  var POSITIONED = ["open", "closed", "fault", "manual", "out-of-service"];
  var DAMPER = {
    properties: [...POSITIONED, "position"],
    deciding: ["fault", "open", "closed", "position"],
    sample: "open",
    states: STANDING((open) => ({ position: { share: open ? 1 : 0 } })).map(
      (state5) => state5.id === "moving" ? { ...state5, values: { ...state5.values, position: { share: 0.5 } } } : state5
    ),
    read: (value, frame, faulted, object) => {
      const position = value("position");
      if (!position) return readFeedback(value("open"), value("closed"), faulted);
      const number2 = Number(position.value);
      const share2 = Number.isFinite(number2) ? Math.min(1, Math.max(0, number2 / 100)) : 0;
      const numbers = readNumbers("position")(value, frame, faulted, object);
      return {
        ...numbers,
        word: faulted ? numbers.word : percentWord(position) ?? "",
        view: { unknown: false, opening: share2 }
      };
    },
    draw: (view) => {
      const angle = round(90 * (1 - (view.opening ?? 0)));
      return [
        el("path", { d: "M0 32 H4 M60 32 H64", fill: "none", ...LINE }),
        el("rect", {
          "data-part": "duct",
          x: 4,
          y: 14,
          width: 56,
          height: 36,
          rx: 2,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        }),
        ...view.unknown ? [] : [
          el("path", {
            "data-part": "blade",
            "data-angle": angle,
            d: `M${MIDDLE - 15} 32 H${MIDDLE + 15}`,
            fill: "none",
            stroke: MOVING,
            "stroke-width": 3.5,
            "stroke-linecap": "round",
            "vector-effect": "non-scaling-stroke",
            transform: angle === 0 ? void 0 : `rotate(${angle} ${MIDDLE} 32)`
          })
        ],
        el("circle", { cx: MIDDLE, cy: 32, r: 3, fill: BODY, ...LINE })
      ];
    }
  };
  var CYLINDER = {
    properties: POSITIONED,
    deciding: ["fault", "open", "closed"],
    sample: "closed",
    states: STANDING(),
    read: (value, _frame, faulted) => readFeedback(value("open"), value("closed"), faulted, { open: "Extended", closed: "Retracted" }),
    draw: (view) => {
      const travel = view.opening ?? 0;
      return [
        el("rect", {
          "data-part": "barrel",
          x: 4,
          y: 8,
          width: 52,
          height: 24,
          rx: 3,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        }),
        el("path", { d: "M12 8 V4 M48 8 V4", fill: "none", ...LINE }),
        ...view.unknown ? [] : [
          el(
            "g",
            {
              "data-part": "rod",
              "data-travel": travel,
              transform: travel === 0 ? void 0 : `translate(${round(36 * travel)} 0)`
            },
            [
              el("rect", { x: 8, y: 11, width: 5, height: 18, rx: 1, fill: MOVING }),
              el("rect", { x: 13, y: 18, width: 43, height: 4, rx: 1, fill: MOVING })
            ]
          )
        ]
      ];
    }
  };
  var MOTOR_VALVE = {
    properties: POSITIONED,
    deciding: ["fault", "open", "closed"],
    sample: "open",
    states: STANDING(),
    read: (value, _frame, faulted) => readFeedback(value("open"), value("closed"), faulted),
    draw: (view) => [
      el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
      el("path", { "data-part": "stem", d: `M${MIDDLE} ${AXIS} V23`, fill: "none", ...LINE }),
      el("g", { "data-part": "motor" }, [
        el("circle", { cx: MIDDLE, cy: 13, r: 10, fill: BODY, ...LINE }),
        text("M", {
          x: MIDDLE,
          y: 17.5,
          fill: token("muted"),
          "font-size": 12,
          "font-weight": 600,
          "text-anchor": "middle"
        })
      ]),
      el("path", {
        "data-part": "body",
        "data-filled": String(view.filled === true),
        d: BODY_PATH,
        fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
        ...LINE
      })
    ]
  };
  var WEIGH_SCALE = {
    properties: ["value", "fault"],
    deciding: ["fault", "value"],
    sample: "weighing",
    states: [
      { id: "weighing", name: "Weighing", values: { value: { share: 0.6 }, fault: false }, needs: ["value"] },
      { id: "fault", name: "Fault", values: { value: { share: 0.6 }, fault: true }, needs: ["fault"] }
    ],
    read: readNumbers("value"),
    draw: (view) => [
      el("rect", {
        "data-part": "platform",
        x: 6,
        y: 26,
        width: 52,
        height: 7,
        rx: 2,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("rect", { "data-part": "load-cell", x: 27, y: 33, width: 10, height: 13, rx: 1, fill: MOVING }),
      el("path", { "data-part": "base", d: "M12 60 L18 46 H46 L52 60 Z", fill: BODY, ...LINE })
    ]
  };

  // src/hmiBuilder/drawing/symbols/electrical.ts
  var WAVE = "q3 -4 6 0 t6 0";
  function converter(view, input) {
    return [
      el("path", { "data-part": "terminals", d: "M32 0 V12 M32 52 V64", fill: "none", ...LINE }),
      el("rect", {
        "data-part": "converter",
        x: 12,
        y: 12,
        width: 40,
        height: 40,
        rx: 2,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("path", { d: "M12 52 L52 12", fill: "none", ...LINE, "stroke-width": 1.5 }),
      input,
      el("path", {
        "data-part": "output",
        d: `M34 41 ${WAVE}`,
        fill: "none",
        ...LINE,
        stroke: paint(view),
        "stroke-linecap": "round"
      })
    ];
  }
  var VFD = {
    properties: ["running", "fault", "manual", "out-of-service", "frequency"],
    deciding: ["fault", "running"],
    sample: "running",
    states: runningStates((running) => ({ frequency: running ? { share: 0.8 } : 0 })),
    read: readRunningWith("frequency", false),
    draw: (view) => converter(
      view,
      el("path", { "data-part": "input", d: `M18 23 ${WAVE}`, fill: "none", ...LINE, "stroke-linecap": "round" })
    )
  };
  var runningDrive = readRunningWith("speed", false);
  var readDrive = (value, frame, faulted, object) => {
    const reading = runningDrive(value, frame, faulted, object);
    const ready = value("ready");
    if (faulted || reading.state === "running" || ready === void 0) return reading;
    return on(ready) ? { ...reading, state: "ready", word: say("Ready") } : { ...reading, state: "not-ready", word: say("Not ready") };
  };
  var DRIVE = {
    properties: ["running", "ready", "fault", "manual", "out-of-service", "speed"],
    deciding: ["fault", "running", "ready"],
    sample: "running",
    states: [
      ...runningStates((running) => ({ ready: true, speed: running ? { share: 0.6 } : 0 })),
      {
        id: "not-ready",
        name: "Not ready",
        values: { running: false, ready: false, fault: false, manual: false, "out-of-service": false, speed: 0 },
        needs: ["ready"]
      }
    ],
    read: readDrive,
    draw: (view) => converter(
      view,
      el("path", {
        "data-part": "input",
        d: "M18 21 H30 M18 25 H30",
        fill: "none",
        ...LINE,
        "stroke-linecap": "round"
      })
    )
  };
  function contact(view, closed, mark) {
    return [
      el("path", { "data-part": "terminals", d: "M32 0 V22 M32 46 V64", fill: "none", ...LINE }),
      el("circle", { cx: 32, cy: 46, r: 2, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("path", {
        "data-part": "contact",
        "data-closed": String(closed),
        d: closed ? "M32 46 L32 22" : "M32 46 L20 25",
        fill: "none",
        ...LINE,
        stroke: closed && !view.unknown ? MOVING : STILL,
        "stroke-width": 3,
        "stroke-linecap": "round"
      }),
      mark
    ];
  }
  function readSwitching(value, faulted) {
    const closed = value("closed");
    const shut = on(closed);
    return {
      state: faulted ? "fault" : closed ? shut ? "closed" : "open" : void 0,
      word: faulted ? say("Fault") : closed ? say(shut ? "Closed" : "Open") : "",
      fault: faulted,
      view: { unknown: false, form: shut ? "closed" : "open" }
    };
  }
  var switchingStates = (extra = {}) => [
    { id: "closed", name: "Closed", values: { closed: true, ...extra }, needs: ["closed"] },
    { id: "open", name: "Open", values: { closed: false, ...extra }, needs: ["closed"] }
  ];
  var CONTACTOR = {
    properties: ["closed", "fault"],
    deciding: ["fault", "closed"],
    sample: "closed",
    states: [
      ...switchingStates({ fault: false }),
      { id: "fault", name: "Fault", values: { closed: false, fault: true }, needs: ["fault"] }
    ],
    read: (value, _frame, faulted) => readSwitching(value, faulted),
    draw: (view) => contact(
      view,
      view.form === "closed",
      el("path", { "data-part": "arc", d: "M32 14 A4 4 0 0 0 32 22", fill: "none", ...LINE })
    )
  };
  var BREAKER = {
    properties: ["closed", "tripped"],
    deciding: ["tripped", "closed"],
    sample: "closed",
    states: [
      ...switchingStates({ tripped: false }),
      { id: "tripped", name: "Tripped", values: { closed: false, tripped: true }, needs: ["tripped"] }
    ],
    read: (value, _frame, faulted) => {
      if (!on(value("tripped"))) return readSwitching(value, faulted);
      return { state: "tripped", word: say("Tripped"), fault: true, view: { unknown: false, form: "open" } };
    },
    draw: (view) => contact(
      view,
      view.form === "closed",
      el("path", { "data-part": "cross", d: "M28.5 18.5 L35.5 25.5 M35.5 18.5 L28.5 25.5", fill: "none", ...LINE })
    )
  };

  // src/hmiBuilder/drawing/symbols/vessels.ts
  function outline({ left, right, top, bottom }) {
    return `M${left} ${top} V${bottom - 3} Q${left} ${bottom} ${left + 3} ${bottom} H${right - 3} Q${right} ${bottom} ${right} ${bottom - 3} V${top}`;
  }
  function vesselOutline(walls) {
    return `${outline(walls)} Z`;
  }
  function roundedRectOutline({ x, y, width, height, rx }) {
    const [right, bottom] = [x + width, y + height];
    const corner = (toX, toY) => `A${rx} ${rx} 0 0 1 ${toX} ${toY}`;
    return `M${x + rx} ${y} H${right - rx} ${corner(right, y + rx)} V${bottom - rx} ${corner(right - rx, bottom)} H${x + rx} ${corner(x, bottom - rx)} V${y + rx} ${corner(x + rx, y)} Z`;
  }
  function openVessel(walls, part, level, unknown) {
    const { left, right, top, bottom } = walls;
    const inside2 = { x: left + 1, y: top + 1, width: right - left - 2, height: bottom - top - 2 };
    const nodes = [
      el("path", { "data-part": "body", d: outline(walls), fill: unknown ? HATCH : BODY, stroke: "none" })
    ];
    if (!unknown && level !== void 0) {
      const height = round(inside2.height * level);
      nodes.push(
        el("rect", {
          "data-part": part,
          "data-level": round(level),
          x: inside2.x,
          y: round(inside2.y + inside2.height - height),
          width: inside2.width,
          height,
          fill: LIQUID
        })
      );
    }
    nodes.push(
      el("path", {
        "data-part": "walls",
        d: `${outline(walls)} M${left - 4} ${top} H${left} M${right} ${top} H${right + 4}`,
        fill: "none",
        ...LINE
      })
    );
    return nodes;
  }
  function roundedLiquid(part, box, level, radius) {
    if (level === void 0 || level <= 0) return [];
    const height = round(box.height * level);
    const top = round(box.y + box.height - height);
    const bottom = box.y + box.height;
    const r = Math.min(radius, height);
    const [left, right] = [box.x, box.x + box.width];
    const d = `M${left} ${top} H${right} V${round(bottom - r)} Q${right} ${bottom} ${round(right - r)} ${bottom} H${round(left + r)} Q${left} ${bottom} ${left} ${round(bottom - r)} Z`;
    return [el("path", { "data-part": part, "data-level": round(level), d, fill: LIQUID })];
  }

  // src/hmiBuilder/drawing/symbols/tank.ts
  var WALLS = { left: 8, right: 56, top: 8, bottom: 54 };
  function levelStates(property = "level") {
    const state5 = (id, name, share2) => ({
      id,
      name,
      values: { [property]: { share: share2 } },
      needs: [property]
    });
    return [state5("low", "Low", 0.15), state5("half", "Half", 0.5), state5("high", "High", 0.9)];
  }
  var TANK = {
    properties: ["level"],
    deciding: ["level"],
    states: levelStates(),
    sample: "half",
    read: readNumbers("level"),
    rules: [{ part: "liquid", level: "level" }],
    outline: vesselOutline(WALLS),
    draw: (view) => [
      ...openVessel(WALLS, "liquid", view.parts?.liquid?.level, view.unknown),
      el("path", { "data-part": "inlet", d: "M20 0 V14", fill: "none", ...LINE }),
      el("path", { "data-part": "outlet", d: "M57 46 H64", fill: "none", ...LINE })
    ]
  };

  // src/hmiBuilder/drawing/symbols/gravityTank.ts
  var WALLS2 = { left: 8, right: 56, top: 8, bottom: 52 };
  var GRAVITY_TANK = {
    properties: ["level"],
    deciding: ["level"],
    states: levelStates(),
    sample: "half",
    read: readNumbers("level"),
    rules: [{ part: "liquid", level: "level" }],
    outline: vesselOutline(WALLS2),
    draw: (view) => [
      ...openVessel(WALLS2, "liquid", view.parts?.liquid?.level, view.unknown),
      el("path", { "data-part": "inlet", d: "M20 0 V14", fill: "none", ...LINE }),
      // The bottom outlet and its orifice: two short bars across the pipe.
      el("path", { "data-part": "outlet", d: "M32 53 V64 M28.5 57.5 H35.5 M28.5 60 H35.5", fill: "none", ...LINE })
    ]
  };

  // src/hmiBuilder/drawing/symbols/instrument.ts
  function bubble(x, letters, unknown, part = "bubble") {
    return [
      el("circle", { "data-part": part, cx: x, cy: 24, r: 20, fill: unknown ? HATCH : BODY, ...LINE }),
      text(letters, {
        "data-part": "letters",
        x,
        y: 29,
        fill: token("text"),
        "font-size": 14,
        "font-weight": 600,
        "text-anchor": "middle"
      })
    ];
  }
  function instrument(letters) {
    return {
      properties: ["value", "fault"],
      deciding: ["fault", "value"],
      states: [
        { id: "measuring", name: "Measuring", values: { value: { share: 0.6 }, fault: false }, needs: ["value"] },
        { id: "fault", name: "Fault", values: { value: { share: 0.6 }, fault: true }, needs: ["fault"] }
      ],
      sample: "measuring",
      read: readNumbers("value"),
      draw: (view) => bubble(24, letters, view.unknown)
    };
  }
  var readPair = readNumbers("a", "b");
  var SENSOR_PAIR = {
    properties: ["a", "b", "disagree"],
    deciding: ["a", "b", "disagree"],
    states: [
      { id: "agree", name: "Agree", values: { a: { share: 0.6 }, b: { share: 0.61 }, disagree: false }, needs: ["a|b"] },
      {
        id: "disagree",
        name: "Disagree",
        values: { a: { share: 0.6 }, b: { share: 0.75 }, disagree: true },
        needs: ["disagree"]
      }
    ],
    sample: "agree",
    // A disagreement is a fault of the measurement: its word, in the alarm colour, with its sign.
    read: (value, frame, faulted, object) => on(value("disagree")) ? { state: "fault", word: say("Disagree"), fault: true, view: { unknown: false } } : readPair(value, frame, faulted, object),
    rules: [{ part: "disagreement", alarm: "disagree" }],
    draw: (view) => {
      const disagree = view.parts?.disagreement?.alarm === true;
      const mark = disagree ? "M50 21 H62 M50 27 H62 M53 32 L59 16" : "M50 21 H62 M50 27 H62";
      return [
        ...bubble(24, "A", view.unknown, "bubble"),
        ...bubble(88, "B", view.unknown, "bubble-b"),
        el("path", {
          "data-part": disagree ? "disagreement" : "agreement",
          d: mark,
          fill: "none",
          ...LINE,
          stroke: disagree ? token("alarm") : token("muted"),
          "stroke-width": disagree ? 2.5 : 2
        })
      ];
    }
  };
  function loopOf(tag) {
    const match = /^([A-Za-z]+)[\s_-]*([0-9][0-9A-Za-z]*)?/.exec(tag ?? "");
    return { letters: match?.[1]?.toUpperCase() ?? "IC", number: match?.[2] ?? "" };
  }
  var readMeasurement = readNumbers("pv");
  var CONTROLLER = {
    properties: ["pv", "sp", "manual"],
    deciding: ["pv", "sp"],
    states: [
      {
        id: "controlling",
        name: "Controlling",
        values: { pv: { share: 0.58 }, sp: { share: 0.62 }, manual: false },
        needs: ["pv"]
      },
      {
        id: "on-setpoint",
        name: "On setpoint",
        values: { pv: { share: 0.62 }, sp: { share: 0.62 }, manual: false },
        needs: ["pv"]
      },
      {
        id: "manual",
        name: "Manual",
        values: { pv: { share: 0.58 }, sp: { share: 0.62 }, manual: true },
        needs: ["manual"]
      }
    ],
    sample: "controlling",
    read: (value, frame, faulted, object) => {
      const reading = readMeasurement(value, frame, faulted, object);
      const [pv, sp] = [shownBoundNumber(value, "pv", frame, object), shownBoundNumber(value, "sp", frame, object)];
      if (faulted || !pv || !sp || pv === sp) return reading;
      const unit = withValueFormat(object, frame, "sp").unit;
      const measured = unit && pv.endsWith(` ${unit}`) && sp.endsWith(` ${unit}`) ? pv.slice(0, -unit.length - 1) : pv;
      return { ...reading, word: `${measured} \u2192 ${sp}` };
    },
    draw: (view) => {
      const { letters, number: number2 } = loopOf(view.tag);
      return [
        el("rect", { "data-part": "frame", x: 2, y: 2, width: 44, height: 44, rx: 3, fill: "none", ...LINE }),
        el("circle", { "data-part": "bubble", cx: 24, cy: 24, r: 20, fill: view.unknown ? HATCH : BODY, ...LINE }),
        el("path", { d: "M4 24 H44", fill: "none", ...LINE }),
        text(letters, {
          "data-part": "letters",
          x: 24,
          y: 19.5,
          fill: token("text"),
          "font-size": letters.length > 3 ? 9 : 11,
          "font-weight": 650,
          "text-anchor": "middle"
        }),
        ...number2 ? [
          text(number2, {
            "data-part": "loop",
            x: 24,
            y: 37,
            fill: token("muted"),
            "font-size": number2.length > 3 ? 8.5 : 10,
            "font-weight": 600,
            "text-anchor": "middle"
          })
        ] : []
      ];
    }
  };
  var ANALYZER = instrument("AT");
  function switchInstrument(letters) {
    return {
      properties: ["state", "fault"],
      deciding: ["fault", "state"],
      states: [
        { id: "off", name: "Off", values: { state: false, fault: false }, needs: ["state"] },
        { id: "on", name: "On", values: { state: true, fault: false }, needs: ["state"] },
        { id: "fault", name: "Fault", values: { state: false, fault: true }, needs: ["fault"] }
      ],
      sample: "off",
      read: (value, _frame, faulted) => {
        const state5 = value("state");
        return {
          state: faulted ? "fault" : state5 ? on(state5) ? "on" : "off" : void 0,
          word: faulted ? say("Fault") : state5 ? say(on(state5) ? "On" : "Off") : "",
          fault: faulted,
          view: { unknown: false }
        };
      },
      draw: (view) => bubble(24, letters, view.unknown)
    };
  }
  var LEVEL_SWITCH = switchInstrument("LS");
  var PRESSURE_SWITCH = switchInstrument("PS");
  var DIGITAL_SWITCH = {
    ...switchInstrument("XS"),
    draw: (view) => bubble(24, /^([A-Za-z]{1,4})/.exec(view.tag ?? "")?.[1]?.toUpperCase() ?? "XS", view.unknown)
  };

  // src/hmiBuilder/drawing/symbols/link.ts
  var LINE_PATH = "M20 32 H44";
  var NETWORK_LINK = {
    properties: ["connected", "age", "hold", "fallback", "out-of-service"],
    deciding: ["connected"],
    states: [
      {
        id: "connected",
        name: "Connected",
        values: { connected: true, age: 120, hold: false, fallback: false, "out-of-service": false },
        needs: ["connected"]
      },
      {
        id: "hold",
        name: "Hold",
        values: { connected: true, age: 900, hold: true, fallback: false, "out-of-service": false },
        needs: ["hold"]
      },
      {
        id: "lost",
        name: "Lost",
        values: { connected: false, age: 4e3, hold: false, fallback: false, "out-of-service": false },
        needs: ["connected"]
      },
      {
        id: "fallback",
        name: "Fallback",
        values: { connected: false, age: 4e3, hold: false, fallback: true, "out-of-service": false },
        needs: ["fallback"]
      },
      {
        id: "out-of-service",
        name: "Out of service",
        values: { connected: false, age: 4e3, hold: false, fallback: false, "out-of-service": true },
        needs: ["out-of-service"]
      }
    ],
    sample: "connected",
    read: (value, frame, faulted, object) => {
      const connected = value("connected");
      if (connected === void 0) return { word: "", fault: faulted, view: { unknown: false } };
      if (!on(connected)) {
        if (on(value("out-of-service")))
          return { state: "out-of-service", word: say("Off"), fault: faulted, view: { unknown: false, form: "off" } };
        return on(value("fallback")) ? { state: "fallback", word: say("Fallback"), fault: faulted, view: { unknown: false, form: "fallback" } } : { state: "lost", word: say("Lost"), fault: true, view: { unknown: false, form: "lost" } };
      }
      if (on(value("hold")))
        return { state: "hold", word: say("Hold"), fault: faulted, view: { unknown: false, form: "hold" } };
      const age = shownBoundNumber(value, "age", frame, object);
      return {
        state: "connected",
        word: faulted ? say("Fault") : age ? `${say("Connected")} \xB7 ${age}` : say("Connected"),
        fault: faulted,
        view: {
          unknown: false,
          form: "connected",
          turning: observed(connected) ? { delayMs: turnDelayMs(frame.nowMs) } : void 0
        }
      };
    },
    draw: (view) => {
      const stations = [
        el("rect", {
          "data-part": "station",
          x: 4,
          y: 22,
          width: 16,
          height: 20,
          rx: 3,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        }),
        el("rect", {
          "data-part": "station",
          x: 44,
          y: 22,
          width: 16,
          height: 20,
          rx: 3,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        })
      ];
      const form = {
        connected: () => [
          el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE }),
          ...view.turning ? [
            el("path", {
              "data-part": "packets",
              class: "hmi-flow",
              style: `animation-delay: -${view.turning.delayMs % 800}ms`,
              d: LINE_PATH,
              fill: "none",
              stroke: MOVING,
              "stroke-width": 3,
              "stroke-dasharray": "2 6",
              "stroke-linecap": "round",
              "vector-effect": "non-scaling-stroke"
            })
          ] : []
        ],
        hold: () => [
          el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE }),
          el("path", {
            "data-part": "hold",
            d: "M29.5 26 V38 M34.5 26 V38",
            fill: "none",
            stroke: MOVING,
            "stroke-width": 3,
            "stroke-linecap": "round",
            "vector-effect": "non-scaling-stroke"
          })
        ],
        off: () => [el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE })],
        lost: () => [
          el("path", { "data-part": "link", d: "M20 32 H27 M37 32 H44", fill: "none", ...LINE }),
          el("path", {
            "data-part": "break",
            d: "M29 28 L35 36 M35 28 L29 36",
            fill: "none",
            stroke: token("alarm"),
            "stroke-width": 2.5,
            "stroke-linecap": "round",
            "vector-effect": "non-scaling-stroke"
          })
        ],
        fallback: () => [
          el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE, "stroke-dasharray": "3 4" }),
          // The local station's control, turning back on itself.
          el("path", {
            "data-part": "fallback",
            d: "M46 20 C44 9 60 9 58 20 M58 20 L54.5 16.5 M58 20 L60.5 15.5",
            fill: "none",
            stroke: MOVING,
            "stroke-width": 2,
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "vector-effect": "non-scaling-stroke"
          })
        ]
      };
      return [
        ...view.unknown || !view.form ? [el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE })] : form[view.form](),
        ...stations
      ];
    }
  };

  // src/hmiBuilder/drawing/symbols/machines.ts
  function turned(part, x, y, children, angle, spin) {
    if (angle !== void 0)
      return el(
        "g",
        { "data-part": part, "data-angle": round(angle), transform: `rotate(${round(angle)} ${x} ${y})` },
        children
      );
    return el(
      "g",
      {
        "data-part": part,
        class: spin ? "hmi-spin" : void 0,
        style: spin ? `animation-delay: -${spin.delayMs}ms` : void 0
      },
      children
    );
  }
  function marker(x, y, length, paint2 = MOVING, width = 2.5) {
    return el("path", {
      d: `M${x} ${y} V${y - length}`,
      fill: "none",
      stroke: paint2,
      "stroke-width": width,
      "stroke-linecap": "round",
      "vector-effect": "non-scaling-stroke"
    });
  }
  function state3(id, name, values) {
    return { id, name, values, needs: [Object.keys(values).join("|")] };
  }
  var readAngle = (value, frame, faulted, object) => {
    const reading = readNumbers("angle")(value, frame, faulted, object);
    return reading.word || faulted ? reading : { ...reading, word: shownBoundNumber(value, "speed", frame, object) ?? "" };
  };
  var MOTOR = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    states: runningStates(),
    sample: "running",
    read: readRunning,
    rules: [{ part: "rotor", spin: "running" }],
    draw: (view) => {
      const rotor2 = view.parts?.rotor;
      return [
        // Its shaft leaves to the right.
        el("path", {
          "data-part": "shaft",
          d: "M50 32 H62",
          fill: "none",
          stroke: LINE.stroke,
          "stroke-width": 4,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        }),
        // IEC 60617: a circle with M, its rotor's mark circling inside it while it runs.
        el("circle", { "data-part": "motor", cx: 28, cy: 32, r: 22, fill: view.unknown ? HATCH : BODY, ...LINE }),
        text("M", { x: 28, y: 38.5, fill: token("muted"), "font-size": 18, "font-weight": 600, "text-anchor": "middle" }),
        ...view.unknown ? [] : [
          turned(
            "rotor",
            28,
            32,
            [
              el("circle", { cx: 28, cy: 32, r: 22, fill: "none", stroke: "none" }),
              el("circle", { cx: 28, cy: 14.5, r: 3, fill: rotor2?.spin ? MOVING : STILL })
            ],
            void 0,
            rotor2?.spin
          )
        ]
      ];
    }
  };
  var SERVO_MOTOR = {
    properties: ["running", "fault", "manual", "out-of-service", "speed", "angle"],
    deciding: ["fault", "running"],
    states: runningStates((running) => ({ speed: running ? { share: 0.5 } : 0, angle: running ? 30 : 0 })),
    sample: "running",
    read: readRunningWith("speed", false),
    rules: [
      { part: "rotor", angle: "angle" },
      { part: "rotor", spin: "running" }
    ],
    draw: (view) => {
      const rotor2 = view.parts?.rotor;
      const moving = rotor2?.angle !== void 0 || rotor2?.spin !== void 0;
      return [
        // The drive (IEC 60617 converter: a box with its diagonal, alternating and direct current).
        el("rect", {
          "data-part": "drive",
          x: 4,
          y: 14,
          width: 30,
          height: 36,
          rx: 3,
          fill: view.unknown ? HATCH : BODY,
          ...LINE
        }),
        el("path", {
          d: "M8 46 L30 18 M9 24 q2 -3 4 0 q2 3 4 0 M22 40 H28 M22 43.5 H28 M34 32 H44 M75 32 H92",
          fill: "none",
          ...LINE
        }),
        // The motor (IEC 60617: a circle with M), its rotor's mark circling inside it.
        el("circle", { "data-part": "motor", cx: 60, cy: 32, r: 15, fill: view.unknown ? HATCH : BODY, ...LINE }),
        text("M", { x: 60, y: 37, fill: token("muted"), "font-size": 14, "font-weight": 600, "text-anchor": "middle" }),
        ...view.unknown ? [] : [
          turned(
            "rotor",
            60,
            32,
            [
              el("circle", { cx: 60, cy: 32, r: 15, fill: "none", stroke: "none" }),
              el("circle", { cx: 60, cy: 21, r: 2.5, fill: moving ? MOVING : STILL })
            ],
            rotor2?.angle,
            rotor2?.spin
          )
        ]
      ];
    }
  };
  var SHAFT = {
    properties: ["angle", "speed"],
    deciding: ["angle", "speed"],
    states: [
      state3("turning", "Turning", { angle: 30, speed: { share: 0.5 } }),
      state3("standing", "Standing", { angle: 0, speed: 0 })
    ],
    sample: "turning",
    read: readAngle,
    rules: [
      { part: "marker", angle: "angle" },
      { part: "marker", spin: "speed" }
    ],
    draw: (view) => {
      const mark = view.parts?.marker;
      return [
        el("path", {
          "data-part": "shaft",
          d: "M2 32 H42",
          fill: "none",
          stroke: LINE.stroke,
          "stroke-width": 4,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        }),
        el("rect", { "data-part": "bearings", x: 10, y: 24, width: 8, height: 16, rx: 2, fill: BODY, ...LINE }),
        el("rect", { x: 28, y: 24, width: 8, height: 16, rx: 2, fill: BODY, ...LINE }),
        el("circle", { "data-part": "end", cx: 50, cy: 32, r: 11, fill: view.unknown ? HATCH : BODY, ...LINE }),
        ...view.unknown ? [] : [
          turned(
            "marker",
            50,
            32,
            [
              el("circle", { cx: 50, cy: 32, r: 11, fill: "none", stroke: "none" }),
              marker(50, 32, 9, mark ? MOVING : STILL)
            ],
            mark?.angle,
            mark?.spin
          )
        ]
      ];
    }
  };
  var DRUM = {
    properties: ["angle", "position"],
    deciding: ["angle", "position"],
    states: [
      state3("raised", "Raised", { angle: 0, position: { share: 0.2 } }),
      state3("lowered", "Lowered", { angle: 270, position: { share: 0.8 } })
    ],
    sample: "raised",
    read: (value, frame, faulted, object) => ({ ...readNumbers("position")(value, frame, faulted, object) }),
    rules: [
      { part: "spokes", angle: "angle" },
      { part: "load", travel: "position" }
    ],
    draw: (view) => {
      const travel = view.parts?.load?.travel ?? 0.3;
      const top = round(30 + travel * 22);
      return [
        el("path", { "data-part": "support", d: "M14 3 H50 M32 3 V7", fill: "none", ...LINE }),
        el("circle", { "data-part": "drum", cx: 32, cy: 18, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
        ...view.unknown ? [] : [
          turned(
            "spokes",
            32,
            18,
            [
              marker(32, 18, 10),
              el("path", {
                d: "M32 18 L40.7 23 M32 18 L23.3 23",
                fill: "none",
                stroke: MOVING,
                "stroke-width": 2.5,
                "stroke-linecap": "round",
                "vector-effect": "non-scaling-stroke"
              })
            ],
            view.parts?.spokes?.angle ?? 0
          ),
          el("path", { "data-part": "rope", d: `M44 18 V${top}`, fill: "none", ...LINE }),
          el("rect", {
            "data-part": "load",
            "data-travel": round(travel),
            x: 35,
            y: top,
            width: 18,
            height: 10,
            rx: 2,
            fill: MOVING
          })
        ]
      ];
    }
  };
  var COUPLING = {
    properties: ["motor-angle", "load-angle"],
    deciding: ["motor-angle", "load-angle"],
    states: [
      state3("twisted", "Twisted", { "motor-angle": 40, "load-angle": 32 }),
      state3("aligned", "Aligned", { "motor-angle": 40, "load-angle": 40 })
    ],
    sample: "twisted",
    read: readNumbers("motor-angle", "load-angle"),
    rules: [
      { part: "motor-marker", angle: "motor-angle" },
      { part: "load-marker", angle: "load-angle" }
    ],
    draw: (view) => [
      el("path", {
        "data-part": "shafts",
        d: "M0 32 H8 M88 32 H96",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 4,
        "vector-effect": "non-scaling-stroke"
      }),
      // The flexible element between the motor's side and the load's.
      el("path", { "data-part": "spring", d: "M32 32 L36 26 L42 38 L48 26 L54 38 L60 26 L64 32", fill: "none", ...LINE }),
      el("circle", { "data-part": "motor-side", cx: 20, cy: 32, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("circle", { "data-part": "load-side", cx: 76, cy: 32, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
      ...view.unknown ? [] : [
        turned("motor-marker", 20, 32, [marker(20, 32, 10)], view.parts?.["motor-marker"]?.angle ?? 0),
        turned("load-marker", 76, 32, [marker(76, 32, 10)], view.parts?.["load-marker"]?.angle ?? 0)
      ]
    ]
  };
  var ENCODER = {
    properties: ["angle"],
    deciding: ["angle"],
    states: [state3("zero", "At 0\xB0", { angle: 0 }), state3("turned", "Turned", { angle: 135 })],
    sample: "turned",
    read: readNumbers("angle"),
    rules: [{ part: "disc", angle: "angle" }],
    draw: (view) => {
      const ticks = Array.from({ length: 12 }, (_, index) => {
        const angle = index * 30 * Math.PI / 180;
        const [inner, outer] = [index === 0 ? 6 : 12, 16];
        return `M${round(28 + inner * Math.sin(angle))} ${round(32 - inner * Math.cos(angle))} L${round(28 + outer * Math.sin(angle))} ${round(32 - outer * Math.cos(angle))}`;
      }).join(" ");
      return [
        el("path", {
          "data-part": "shaft",
          d: "M0 32 H10",
          fill: "none",
          stroke: LINE.stroke,
          "stroke-width": 4,
          "vector-effect": "non-scaling-stroke"
        }),
        el("circle", { "data-part": "wheel", cx: 28, cy: 32, r: 19, fill: view.unknown ? HATCH : BODY, ...LINE }),
        ...view.unknown ? [] : [
          turned(
            "disc",
            28,
            32,
            [
              el("path", {
                d: ticks,
                fill: "none",
                stroke: MOVING,
                "stroke-width": 1.5,
                "stroke-linecap": "round",
                "vector-effect": "non-scaling-stroke"
              })
            ],
            view.parts?.disc?.angle ?? 0
          )
        ],
        // The read head over the wheel's rim.
        el("rect", { "data-part": "head", x: 43, y: 25, width: 14, height: 14, rx: 2, fill: BODY, ...LINE })
      ];
    }
  };
  var ROTARY_ARM = {
    properties: ["angle"],
    deciding: ["angle"],
    states: [state3("hanging", "Hanging", { angle: 0 }), state3("raised", "Raised", { angle: 60 })],
    sample: "raised",
    read: readNumbers("angle"),
    rules: [{ part: "arm", angle: "angle" }],
    // The joint in the middle, held from the wall at the left, so the arm can turn all the way
    // round within the symbol.
    draw: (view) => [
      el("path", { "data-part": "mount", d: "M5 20 V40 M5 30 H28", fill: "none", ...LINE }),
      // Gravity, against which the joint works.
      el("path", { "data-part": "gravity", d: "M57 44 V60 M53.5 56.5 L57 60 L60.5 56.5", fill: "none", ...LINE }),
      ...view.unknown ? [el("circle", { cx: 32, cy: 30, r: 4, fill: HATCH, ...LINE })] : [
        turned(
          "arm",
          32,
          30,
          [
            el("path", {
              d: "M32 30 V46",
              fill: "none",
              stroke: MOVING,
              "stroke-width": 3,
              "stroke-linecap": "round",
              "vector-effect": "non-scaling-stroke"
            }),
            el("circle", { cx: 32, cy: 50, r: 6, fill: MOVING })
          ],
          view.parts?.arm?.angle ?? 0
        ),
        el("circle", { "data-part": "pivot", cx: 32, cy: 30, r: 4, fill: BODY, ...LINE })
      ]
    ]
  };
  var AXIS_INDICATOR = {
    properties: ["actual", "target"],
    deciding: ["actual", "target"],
    states: [
      state3("on-target", "On target", { actual: 90, target: 90 }),
      state3("following", "Following", { actual: 75, target: 90 })
    ],
    sample: "following",
    // The actual angle, and the target beside it while the axis has not reached it.
    read: (value, frame, faulted, object) => {
      const reading = readNumbers("actual")(value, frame, faulted, object);
      const [actual, target] = [
        shownBoundNumber(value, "actual", frame, object),
        shownBoundNumber(value, "target", frame, object)
      ];
      return !faulted && actual && target && actual !== target ? { ...reading, word: `${actual} \u2192 ${target}` } : reading;
    },
    rules: [
      { part: "actual", angle: "actual" },
      { part: "target", angle: "target" }
    ],
    draw: (view) => {
      const ticks = Array.from({ length: 12 }, (_, index) => {
        const angle = index * 30 * Math.PI / 180;
        return `M${round(32 + 22 * Math.sin(angle))} ${round(32 - 22 * Math.cos(angle))} L${round(32 + 26 * Math.sin(angle))} ${round(32 - 26 * Math.cos(angle))}`;
      }).join(" ");
      return [
        el("circle", { "data-part": "dial", cx: 32, cy: 32, r: 28, fill: view.unknown ? HATCH : BODY, ...LINE }),
        el("path", { "data-part": "ticks", d: ticks, fill: "none", ...LINE }),
        ...view.unknown ? [] : [
          // The target: a dashed line to a pointer on the rim.
          ...view.parts?.target?.angle !== void 0 ? [
            turned(
              "target",
              32,
              32,
              [
                el("path", { d: "M32 32 V10", fill: "none", ...LINE, "stroke-dasharray": "3 3" }),
                el("path", { d: "M27.5 1 L36.5 1 L32 7.5 Z", fill: token("muted") })
              ],
              view.parts.target.angle
            )
          ] : [],
          turned("actual", 32, 32, [marker(32, 32, 21, MOVING, 3)], view.parts?.actual?.angle ?? 0),
          el("circle", { cx: 32, cy: 32, r: 3.5, fill: MOVING })
        ]
      ];
    }
  };

  // src/hmiBuilder/drawing/symbols/mixer.ts
  var MIXER = {
    properties: ["ratio"],
    deciding: ["ratio"],
    states: [{ id: "mixing", name: "Mixing", values: { ratio: { share: 0.5 } }, needs: ["ratio"] }],
    sample: "mixing",
    read: readNumbers("ratio"),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M0 32 H13 M51 32 H64 M24 43 V64", fill: "none", ...LINE }),
      el("rect", {
        "data-part": "body",
        x: 14,
        y: 22,
        width: 36,
        height: 20,
        rx: 4,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("path", {
        "data-part": "elements",
        d: "M18 32 L22 26 L27 38 L32 26 L37 38 L42 26 L46 32",
        fill: "none",
        ...LINE
      })
    ]
  };

  // src/hmiBuilder/drawing/symbols/process.ts
  var MACHINE2 = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    sample: "running"
  };
  var VESSEL_LEFT = 14;
  var VESSEL_RIGHT = 50;
  var HEAD_RX = 18;
  var HEAD_RY = 10;
  var VESSEL_TOP = 4;
  var VESSEL_BOTTOM = 60;
  var VESSEL_OUTLINE = `M${VESSEL_LEFT} 14 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_RIGHT} 14 V50 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_LEFT} 50 Z`;
  function headHalfWidth(y, centre) {
    const along = Math.min(1, Math.abs(y - centre) / HEAD_RY);
    return round(HEAD_RX * Math.sqrt(1 - along * along));
  }
  function vesselLiquid(level) {
    const surface = round(VESSEL_BOTTOM - (VESSEL_BOTTOM - VESSEL_TOP) * Math.min(1, Math.max(0, level)));
    if (surface >= 50) {
      const half2 = headHalfWidth(surface, 50);
      return `M${round(32 - half2)} ${surface} H${round(32 + half2)} A${HEAD_RX} ${HEAD_RY} 0 0 1 ${round(32 - half2)} ${surface} Z`;
    }
    const lower = `V50 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_LEFT} 50`;
    if (surface >= 14) return `M${VESSEL_LEFT} ${surface} H${VESSEL_RIGHT} ${lower} Z`;
    const half = headHalfWidth(surface, 14);
    return `M${round(32 - half)} ${surface} H${round(32 + half)} A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_RIGHT} 14 ${lower} V14 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${round(32 - half)} ${surface} Z`;
  }
  var VESSEL = {
    properties: ["level"],
    deciding: ["level"],
    states: levelStates(),
    sample: "half",
    read: readNumbers("level"),
    rules: [{ part: "liquid", level: "level" }],
    outline: VESSEL_OUTLINE,
    draw: (view) => {
      const level = view.parts?.liquid?.level;
      return [
        el("path", { "data-part": "nozzles", d: `M32 0 V${VESSEL_TOP} M32 ${VESSEL_BOTTOM} V64`, fill: "none", ...LINE }),
        el("path", { "data-part": "body", d: VESSEL_OUTLINE, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
        ...view.unknown || level === void 0 || level <= 0 ? [] : [el("path", { "data-part": "liquid", "data-level": round(level), d: vesselLiquid(level), fill: LIQUID })],
        el("path", {
          "data-part": "heads",
          d: `M${VESSEL_LEFT} 14 H${VESSEL_RIGHT} M${VESSEL_LEFT} 50 H${VESSEL_RIGHT}`,
          fill: "none",
          ...LINE,
          "stroke-width": 1,
          "stroke-dasharray": "3 3"
        }),
        el("path", { "data-part": "vessel", d: VESSEL_OUTLINE, fill: "none", ...LINE })
      ];
    }
  };
  var HEAT_EXCHANGER = {
    properties: ["temperature", "fault"],
    deciding: ["fault", "temperature"],
    sample: "working",
    states: [
      { id: "working", name: "Working", values: { temperature: { share: 0.6 }, fault: false }, needs: ["temperature"] },
      { id: "fault", name: "Fault", values: { temperature: { share: 0.6 }, fault: true }, needs: ["fault"] }
    ],
    read: readNumbers("temperature"),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M0 32 H10 M54 32 H64", fill: "none", ...LINE }),
      el("circle", { "data-part": "shell", cx: 32, cy: 32, r: 22, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("path", {
        "data-part": "coil",
        d: "M32 0 V14 L22 20 L42 28 L22 36 L42 44 L32 50 V64",
        fill: "none",
        ...LINE,
        "stroke-linejoin": "round"
      })
    ]
  };
  var FILTER = {
    properties: ["differential-pressure", "clogged", "fault"],
    deciding: ["fault", "clogged", "differential-pressure"],
    sample: "clean",
    states: [
      {
        id: "clean",
        name: "Clean",
        values: { "differential-pressure": { share: 0.2 }, clogged: false, fault: false },
        needs: ["differential-pressure|clogged"]
      },
      {
        id: "clogged",
        name: "Clogged",
        values: { "differential-pressure": { share: 0.9 }, clogged: true, fault: false },
        needs: ["clogged"]
      },
      {
        id: "fault",
        name: "Fault",
        values: { "differential-pressure": { share: 0.2 }, clogged: false, fault: true },
        needs: ["fault"]
      }
    ],
    read: (value, frame, faulted, object) => {
      const numbers = readNumbers("differential-pressure")(value, frame, faulted, object);
      if (faulted || !on(value("clogged"))) return numbers;
      return { state: "clogged", word: say("Clogged"), fault: false, view: { unknown: false, form: "clogged" } };
    },
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M0 32 H6 M58 32 H64", fill: "none", ...LINE }),
      el("path", {
        "data-part": "housing",
        d: "M6 32 L32 8 L58 32 L32 56 Z",
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
        "stroke-linejoin": "round"
      }),
      el("path", {
        "data-part": "medium",
        d: "M32 10 V54",
        fill: "none",
        ...LINE,
        stroke: view.form === "clogged" ? token("warning") : LINE.stroke,
        "stroke-width": view.form === "clogged" ? 3 : 2,
        "stroke-dasharray": "4 3"
      })
    ]
  };
  var CYCLONE = {
    properties: ["differential-pressure", "fault"],
    deciding: ["fault", "differential-pressure"],
    sample: "working",
    states: [
      {
        id: "working",
        name: "Working",
        values: { "differential-pressure": { share: 0.5 }, fault: false },
        needs: ["differential-pressure"]
      },
      { id: "fault", name: "Fault", values: { "differential-pressure": { share: 0.5 }, fault: true }, needs: ["fault"] }
    ],
    read: readNumbers("differential-pressure"),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M0 14 H18 M32 0 V6 M32 58 V64", fill: "none", ...LINE }),
      el("path", {
        "data-part": "body",
        d: "M18 6 H46 V26 L35 58 H29 L18 26 Z",
        fill: view.unknown ? HATCH : BODY,
        stroke: "none"
      }),
      el("path", { "data-part": "cylinder", d: "M18 26 V6 H46 V26", fill: "none", ...LINE, "stroke-linejoin": "round" }),
      el("path", {
        "data-part": "cone",
        d: "M18 26 L29 58 H35 L46 26 Z",
        fill: "none",
        ...LINE,
        "stroke-linejoin": "round"
      }),
      // The vortex finder: the gas outlet's tube reaching into the cylinder.
      el("path", { "data-part": "vortex-finder", d: "M28 6 V20 M36 6 V20", fill: "none", ...LINE, "stroke-width": 1.5 })
    ]
  };
  var CRUSHER = {
    ...MACHINE2,
    properties: [...MACHINE2.properties, "load"],
    read: readRunningWith("load", true),
    states: runningStates((running) => ({ load: running ? 72 : 0 })),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M32 0 V12 M32 58 V64", fill: "none", ...LINE }),
      el("path", {
        "data-part": "housing",
        d: "M10 12 H54 V48 L44 58 H20 L10 48 Z",
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
        "stroke-linejoin": "round"
      }),
      el("path", {
        "data-part": "breaker-plate",
        d: "M54 20 L48 24 L54 28 L48 32 L54 36 L48 40",
        fill: "none",
        ...LINE,
        "stroke-linejoin": "round"
      }),
      ...view.unknown ? [] : [
        rotor(30, 34, 15, view, [
          ...[0, 90, 180, 270].map(
            (angle) => el("rect", {
              x: 28,
              y: 20,
              width: 4,
              height: 9,
              rx: 1,
              fill: paint(view),
              transform: `rotate(${angle} 30 34)`
            })
          ),
          el("circle", { cx: 30, cy: 34, r: 6, fill: BODY, ...LINE })
        ])
      ]
    ]
  };
  var SCREEN = {
    ...MACHINE2,
    read: readRunning,
    states: runningStates(),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M8 0 V21 M58 44 H64 M32 47 V64", fill: "none", ...LINE }),
      el("path", {
        "data-part": "deck",
        d: "M6 20 L58 40 V48 L6 28 Z",
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
        "stroke-linejoin": "round"
      }),
      el("path", {
        "data-part": "mesh",
        d: "M9 25 L55 43",
        fill: "none",
        ...LINE,
        "stroke-width": 1.5,
        "stroke-dasharray": "3 3"
      }),
      el("path", {
        "data-part": "springs",
        d: "M14 31 L11 34 L17 37 L11 40 L14 42 M50 45 L47 48 L53 51 L47 54 L50 56",
        fill: "none",
        ...LINE,
        "stroke-width": 1.5,
        "stroke-linejoin": "round"
      }),
      el("path", { "data-part": "mount", d: "M32 22 V30", fill: "none", ...LINE }),
      el("circle", { "data-part": "exciter", cx: 32, cy: 14, r: 8, fill: BODY, ...LINE }),
      ...view.unknown ? [] : [rotor(32, 14, 6, view, [el("circle", { cx: 32, cy: 9.5, r: 2.5, fill: paint(view) })])]
    ]
  };

  // src/hmiBuilder/drawing/symbols/pump.ts
  var CENTRE = { x: 30, y: 36 };
  var VANES = 6;
  var PUMP = {
    properties: ["running", "fault", "manual", "out-of-service", "speed"],
    deciding: ["fault", "running"],
    sample: "running",
    // A variable-speed pump gives its speed while it runs.
    read: readRunningWith("speed", true),
    states: runningStates((running) => ({ speed: running ? 62 : 0 })),
    draw: (view) => [
      // A variable-speed pump: the ISO 14617 arrow of variability, passing behind the casing.
      ...view.variant === "variable-speed" ? [
        el("path", {
          "data-part": "variable",
          d: "M3 61 L59 5 M59 5 L51.3 7.1 M59 5 L56.9 12.7",
          fill: "none",
          ...LINE
        })
      ] : [],
      el("path", { "data-part": "inlet", d: `M0 ${CENTRE.y} H10`, fill: "none", ...LINE }),
      el("path", { "data-part": "stand", d: "M21 51 L16 62 H44 L39 51 Z", fill: BODY, ...LINE }),
      // The casing and its outlet in one outline: the outlet leaves the top of the casing along its
      // tangent and ends at the symbol's right edge.
      el("path", {
        "data-part": "casing",
        d: "M30 16 H64 V24 H46 A20 20 0 1 1 30 16 Z",
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      ...view.unknown ? [] : [impeller(view)]
    ]
  };
  function impeller(view) {
    const paint2 = view.turning ? MOVING : STILL;
    const point2 = (radius, degrees) => {
      const angle = degrees * Math.PI / 180;
      return `${round(CENTRE.x + radius * Math.cos(angle))} ${round(CENTRE.y + radius * Math.sin(angle))}`;
    };
    const vanes = Array.from({ length: VANES }, (_, index) => {
      const root = 360 / VANES * index;
      return `M${point2(5.5, root)} Q${point2(11, root - 12)} ${point2(14, root - 42)}`;
    }).join(" ");
    return el(
      "g",
      {
        "data-part": "impeller",
        class: view.turning ? "hmi-spin" : void 0,
        style: view.turning ? `animation-delay: -${view.turning.delayMs}ms` : void 0
      },
      [
        el("circle", { cx: CENTRE.x, cy: CENTRE.y, r: 15, fill: "none", stroke: "none" }),
        el("path", {
          d: vanes,
          fill: "none",
          stroke: paint2,
          "stroke-width": 2.5,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        }),
        el("circle", { cx: CENTRE.x, cy: CENTRE.y, r: 3.5, fill: paint2 })
      ]
    );
  }

  // src/hmiBuilder/drawing/symbols/supply.ts
  var SUPPLY = {
    properties: ["pressure"],
    deciding: ["pressure"],
    states: [
      { id: "normal", name: "Normal", values: { pressure: { share: 0.6 } }, needs: ["pressure"] },
      { id: "low", name: "Low", values: { pressure: { share: 0.1 } }, needs: ["pressure"] }
    ],
    sample: "normal",
    read: readNumbers("pressure"),
    draw: (view) => [
      el("path", {
        "data-part": "connector",
        d: "M4 22 H22 L32 32 L22 42 H4 Z",
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      ...view.variant === "regulated" ? [
        el("g", { "data-part": "regulator" }, [
          el("path", { d: "M33 32 H38 M58 32 H64 M48 32 V20 M55 20 H61 V31", fill: "none", ...LINE }),
          el("path", { d: "M38 25 L48 32 L38 39 Z M58 25 L48 32 L58 39 Z", fill: BODY, ...LINE }),
          el("path", { d: "M41 20 A7 6 0 0 1 55 20 Z", fill: BODY, ...LINE })
        ])
      ] : [el("path", { "data-part": "line", d: "M33 32 H64", fill: "none", ...LINE })]
    ]
  };

  // src/hmiBuilder/drawing/symbols/tankCascade.ts
  var UPPER = { left: 6, right: 42, top: 8, bottom: 30 };
  var LOWER = { left: 54, right: 90, top: 40, bottom: 66 };
  function state4(id, name, upper, lower) {
    return {
      id,
      name,
      values: { "upper-level": { share: upper }, "lower-level": { share: lower } },
      needs: ["upper-level|lower-level"]
    };
  }
  var TANK_CASCADE = {
    properties: ["upper-level", "lower-level"],
    deciding: ["upper-level", "lower-level"],
    states: [
      state4("upper-full", "Upper full", 0.8, 0.3),
      state4("even", "Even", 0.5, 0.5),
      state4("lower-full", "Lower full", 0.2, 0.85)
    ],
    sample: "even",
    read: readNumbers("upper-level", "lower-level"),
    rules: [
      { part: "upper-liquid", level: "upper-level" },
      { part: "lower-liquid", level: "lower-level" }
    ],
    outline: `${vesselOutline(UPPER)} ${vesselOutline(LOWER)}`,
    draw: (view) => [
      ...openVessel(UPPER, "upper-liquid", view.parts?.["upper-liquid"]?.level, view.unknown),
      ...openVessel(LOWER, "lower-liquid", view.parts?.["lower-liquid"]?.level, view.unknown),
      el("path", { "data-part": "inlet", d: "M14 0 V12", fill: "none", ...LINE }),
      // The upper tank drains from its bottom into the lower one, which it enters from above.
      el("path", { "data-part": "drain", d: "M24 31 V35 H66 V46", fill: "none", ...LINE }),
      el("path", { "data-part": "outlet", d: "M91 60 H96", fill: "none", ...LINE })
    ]
  };

  // src/hmiBuilder/drawing/symbols/thermal.ts
  var HEATER_VESSEL = { x: 8, y: 6, width: 48, height: 52, rx: 6 };
  var JACKET = "M10 22 V50 Q10 60 20 60 H44 Q54 60 54 50 V22 Z";
  var JACKETED = { x: 16, y: 6, width: 32, height: 50, rx: 8 };
  var COUPLED = { x: 6, y: 6, width: 84, height: 52, rx: 6 };
  function fan(x, y, radius, spin) {
    const paint2 = spin ? MOVING : STILL;
    const point2 = (r, degrees) => {
      const angle = degrees * Math.PI / 180;
      return `${round(x + r * Math.cos(angle))} ${round(y + r * Math.sin(angle))}`;
    };
    const vanes = [0, 120, 240].map((root) => `M${point2(3, root)} Q${point2(radius * 0.7, root - 10)} ${point2(radius * 0.85, root - 50)}`).join(" ");
    return el(
      "g",
      {
        "data-part": "fan",
        class: spin ? "hmi-spin" : void 0,
        style: spin ? `animation-delay: -${spin.delayMs}ms` : void 0
      },
      [
        el("circle", { cx: x, cy: y, r: radius, fill: "none", stroke: "none" }),
        el("path", {
          d: vanes,
          fill: "none",
          stroke: paint2,
          "stroke-width": 2.5,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        }),
        el("circle", { cx: x, cy: y, r: 2.5, fill: paint2 })
      ]
    );
  }
  function heat(xs, y) {
    const waves = xs.map((x) => `M${x} ${y} q3 -3 0 -6 q-3 -3 0 -6`).join(" ");
    return el("path", {
      "data-part": "heat",
      d: waves,
      fill: "none",
      stroke: MOVING,
      "stroke-width": 1.5,
      "stroke-linecap": "round",
      "vector-effect": "non-scaling-stroke"
    });
  }
  var readTemperature = (value, frame, faulted, object) => {
    const reading = readNumbers("temperature")(value, frame, faulted, object);
    return reading.word || faulted ? reading : { ...reading, word: percentWord(value("level")) ?? percentWord(value("power")) ?? "" };
  };
  var DOOR = { hinge: [76, 46], length: 20, opens: 75 };
  var COLD_ROOM = {
    properties: ["door-open", "temperature"],
    deciding: ["temperature", "door-open"],
    states: [
      {
        id: "closed",
        name: "Door shut",
        values: { "door-open": false, temperature: { share: 0.25 } },
        needs: ["door-open"]
      },
      { id: "open", name: "Door open", values: { "door-open": true, temperature: { share: 0.4 } }, needs: ["door-open"] }
    ],
    sample: "closed",
    read: (value, frame, faulted, object) => {
      const reading = readNumbers("temperature")(value, frame, faulted, object);
      const open = value("door-open");
      return {
        ...reading,
        state: open === void 0 ? reading.state : open.value === true ? "door-open" : "door-shut",
        view: { ...reading.view, opening: open?.value === true ? 1 : 0 }
      };
    },
    draw: (view) => {
      const [hx, hy] = DOOR.hinge;
      const angle = (view.opening ?? 0) * DOOR.opens * Math.PI / 180;
      const end2 = [round(hx + DOOR.length * Math.sin(angle)), round(hy - DOOR.length * Math.cos(angle))];
      const open = (view.opening ?? 0) > 0;
      return [
        el("path", { "data-part": "body", d: "M6 6 H78 V58 H6 Z", fill: view.unknown ? HATCH : BODY, stroke: "none" }),
        // Insulated walls, open where the door is.
        el("path", {
          "data-part": "walls",
          d: "M78 26 V6 H6 V58 H78 V46 M74 26 V10 H10 V54 H74 V46 M74 26 H78 M74 46 H78",
          fill: "none",
          ...LINE
        }),
        // The snowflake that marks a cold room.
        el("path", {
          "data-part": "cold",
          d: "M30 23 V41 M22.2 27.5 L37.8 36.5 M22.2 36.5 L37.8 27.5",
          fill: "none",
          ...LINE
        }),
        ...view.unknown ? [] : [
          ...open ? [
            el("path", {
              "data-part": "swing",
              d: `M${hx} ${hy - DOOR.length} A${DOOR.length} ${DOOR.length} 0 0 1 ${end2[0]} ${end2[1]}`,
              fill: "none",
              ...LINE,
              "stroke-dasharray": "3 3"
            })
          ] : [],
          el("path", {
            "data-part": "door",
            "data-open": String(open),
            d: `M${hx} ${hy} L${end2[0]} ${end2[1]}`,
            fill: "none",
            stroke: open ? MOVING : STILL,
            "stroke-width": 3,
            "stroke-linecap": "round",
            "vector-effect": "non-scaling-stroke"
          })
        ]
      ];
    }
  };
  var REFRIGERATION_UNIT = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    states: runningStates(),
    sample: "running",
    read: readRunning,
    rules: [{ part: "fan", spin: "running" }],
    draw: (view) => [
      el("rect", {
        "data-part": "housing",
        x: 4,
        y: 8,
        width: 88,
        height: 48,
        rx: 4,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      // The compressor (ISO 10628-2): a circle with its two lines narrowing in the flow's direction.
      el("circle", { "data-part": "compressor", cx: 28, cy: 32, r: 15, fill: BODY, ...LINE }),
      el("path", { d: "M15.5 24 L40 28.5 M15.5 40 L40 35.5", fill: "none", ...LINE }),
      el("circle", { "data-part": "shroud", cx: 66, cy: 32, r: 15, fill: BODY, ...LINE }),
      ...view.unknown ? [] : [fan(66, 32, 13, view.parts?.fan?.spin)]
    ]
  };
  var HEATER = {
    properties: ["power", "temperature", "level"],
    deciding: ["power", "temperature"],
    states: [
      {
        id: "heating",
        name: "Heating",
        values: { power: 40, temperature: { share: 0.55 }, level: 80 },
        needs: ["power"]
      },
      { id: "idle", name: "Idle", values: { power: 0, temperature: { share: 0.3 }, level: 80 }, needs: ["power"] }
    ],
    sample: "heating",
    read: readTemperature,
    rules: [
      { part: "element", active: "power" },
      { part: "liquid", level: "level", range: [0, 100] }
    ],
    outline: roundedRectOutline(HEATER_VESSEL),
    draw: (view) => {
      const active = view.parts?.element?.active === true;
      return [
        el("rect", { "data-part": "body", ...HEATER_VESSEL, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
        ...view.unknown ? [] : roundedLiquid("liquid", { x: 9, y: 7, width: 46, height: 50 }, view.parts?.liquid?.level, 5),
        el("rect", { "data-part": "vessel", ...HEATER_VESSEL, fill: "none", ...LINE }),
        el("path", { "data-part": "pipes", d: "M20 0 V6 M57 16 H64", fill: "none", ...LINE }),
        // The heating element: a coil across the bottom, its leads through the wall.
        el("path", {
          "data-part": "element",
          "data-active": String(active),
          d: "M8 48 H13 L17 42 L21 50 L25 42 L29 50 L33 42 L37 50 L41 42 L45 50 L49 42 L51 46",
          fill: "none",
          stroke: active ? MOVING : STILL,
          "stroke-width": 2.5,
          "stroke-linejoin": "round",
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke"
        }),
        ...active && !view.unknown ? [heat([22, 32, 42], 37)] : []
      ];
    }
  };
  var COOLER = {
    properties: ["running", "fault", "manual", "out-of-service"],
    deciding: ["fault", "running"],
    states: runningStates(),
    sample: "running",
    read: readRunning,
    rules: [{ part: "fan", spin: "running" }],
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M0 18 H6 M58 18 H64", fill: "none", ...LINE }),
      // The tube bundle the air cools, and its fan beneath (ISO 10628-2 air cooler).
      el("rect", {
        "data-part": "bundle",
        x: 6,
        y: 10,
        width: 52,
        height: 16,
        rx: 2,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      el("path", { d: "M11 15 H53 M11 21 H53", fill: "none", ...LINE }),
      el("circle", { "data-part": "shroud", cx: 32, cy: 45, r: 14, fill: BODY, ...LINE }),
      el("path", { d: "M32 31 V26", fill: "none", ...LINE }),
      ...view.unknown ? [] : [fan(32, 45, 12, view.parts?.fan?.spin)]
    ]
  };
  var JACKETED_VESSEL = {
    properties: ["level", "temperature"],
    deciding: ["level", "temperature"],
    states: [
      {
        id: "filled",
        name: "Filled",
        values: { level: 60, temperature: { share: 0.5 } },
        needs: ["level|temperature"]
      }
    ],
    sample: "filled",
    read: readTemperature,
    rules: [{ part: "liquid", level: "level", range: [0, 100] }],
    outline: `${JACKET} ${roundedRectOutline(JACKETED)}`,
    draw: (view) => [
      // The jacket around the lower part of the vessel, then the vessel over it.
      el("path", { "data-part": "jacket", d: JACKET, fill: BODY, ...LINE }),
      el("path", { "data-part": "pipes", d: "M32 0 V6 M32 61 V64 M0 52 H9 M55 26 H64", fill: "none", ...LINE }),
      el("rect", { "data-part": "body", ...JACKETED, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
      ...view.unknown ? [] : roundedLiquid("liquid", { x: 17, y: 7, width: 30, height: 48 }, view.parts?.liquid?.level, 7),
      el("rect", { "data-part": "vessel", ...JACKETED, fill: "none", ...LINE })
    ]
  };
  var COUPLED_VESSELS = {
    properties: ["temperature-1", "temperature-2"],
    deciding: ["temperature-1", "temperature-2"],
    states: [
      {
        id: "left-warmer",
        name: "Left warmer",
        values: { "temperature-1": { share: 0.7 }, "temperature-2": { share: 0.4 } },
        needs: ["temperature-1|temperature-2"]
      },
      {
        id: "even",
        name: "Even",
        values: { "temperature-1": { share: 0.5 }, "temperature-2": { share: 0.5 } },
        needs: ["temperature-1|temperature-2"]
      }
    ],
    sample: "left-warmer",
    // The heat passes the wall from the warmer vessel to the colder, when they differ by at least a
    // hundredth of the object's range.
    read: (value, frame, faulted, object) => {
      const reading = readNumbers("temperature-1", "temperature-2")(value, frame, faulted, object);
      const [left, right] = [Number(value("temperature-1")?.value), Number(value("temperature-2")?.value)];
      const least = ((object.max ?? 100) - (object.min ?? 0)) / 100;
      if (!Number.isFinite(left) || !Number.isFinite(right) || Math.abs(left - right) < least) return reading;
      return { ...reading, view: { ...reading.view, direction: left > right ? "right" : "left" } };
    },
    outline: roundedRectOutline(COUPLED),
    draw: (view) => [
      el("path", { "data-part": "pipes", d: "M20 0 V6 M20 58 V64 M76 0 V6 M76 58 V64", fill: "none", ...LINE }),
      el("rect", { "data-part": "vessels", ...COUPLED, fill: view.unknown ? HATCH : BODY, ...LINE }),
      // The wall the heat passes through, drawn heavy.
      el("path", {
        "data-part": "wall",
        d: "M48 7 V57",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 4,
        "vector-effect": "non-scaling-stroke"
      }),
      ...view.direction && !view.unknown ? [heatFlow(view.direction)] : []
    ]
  };
  function heatFlow(direction) {
    const [from, to, back] = direction === "right" ? [40, 56, 52] : [56, 40, 44];
    const arrows = [20, 32, 44].map((y) => `M${from} ${y} H${to} M${back} ${y - 3} L${to} ${y} L${back} ${y + 3}`).join(" ");
    return el("path", {
      "data-part": "heat-flow",
      "data-direction": direction,
      d: arrows,
      fill: "none",
      stroke: MOVING,
      "stroke-width": 1.5,
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      "vector-effect": "non-scaling-stroke"
    });
  }
  var HEAT_DRAW_OFF = {
    properties: ["power"],
    deciding: ["power"],
    states: [
      { id: "drawing", name: "Drawing heat", values: { power: { share: 0.4 } }, needs: ["power"] },
      { id: "idle", name: "Idle", values: { power: 0 }, needs: ["power"] }
    ],
    sample: "drawing",
    read: (value, frame, faulted, object) => ({
      ...readNumbers("power")(value, frame, faulted, object),
      word: shownBoundNumber(value, "power", frame, object) ?? ""
    }),
    rules: [{ part: "heat", active: "power" }],
    draw: (view) => [
      // A finned radiator the heat leaves through, and the heat rising from it while it draws.
      el("path", { "data-part": "pipes", d: "M0 44 H12 M52 44 H64", fill: "none", ...LINE }),
      el("path", {
        "data-part": "fins",
        d: "M17 39 V31 M23 39 V31 M29 39 V31 M35 39 V31 M41 39 V31 M47 39 V31",
        fill: "none",
        ...LINE
      }),
      el("rect", {
        "data-part": "radiator",
        x: 12,
        y: 39,
        width: 40,
        height: 10,
        rx: 2,
        fill: view.unknown ? HATCH : BODY,
        ...LINE
      }),
      ...view.parts?.heat?.active && !view.unknown ? [heat([20, 32, 44], 26)] : []
    ]
  };

  // src/hmiBuilder/drawing/symbols/geometry.ts
  var LABEL_HEIGHT = 44;
  var SYMBOL_GEOMETRY = {
    agitator: { size: [64, 64], ports: {} },
    analyzer: { size: [48, 48], label: "beside", ports: {} },
    "axis-indicator": { size: [64, 64], ports: {} },
    blower: {
      size: [64, 64],
      ports: { inlet: { at: [0, 32], direction: "left" }, outlet: { at: [64, 32], direction: "right" } }
    },
    breaker: { size: [64, 64], ports: {} },
    "cold-room": { size: [96, 64], ports: {} },
    contactor: { size: [64, 64], ports: {} },
    "control-valve": {
      size: [64, 64],
      ports: { inlet: { at: [0, 44], direction: "left" }, outlet: { at: [64, 44], direction: "right" } }
    },
    controller: { size: [48, 48], label: "beside", ports: {} },
    conveyor: { size: [96, 40], ports: {} },
    cooler: {
      size: [64, 64],
      ports: { inlet: { at: [0, 18], direction: "left" }, outlet: { at: [64, 18], direction: "right" } }
    },
    "coupled-vessels": {
      size: [96, 64],
      ports: {
        "inlet-1": { at: [20, 0], direction: "up" },
        "outlet-1": { at: [20, 64], direction: "down" },
        "inlet-2": { at: [76, 0], direction: "up" },
        "outlet-2": { at: [76, 64], direction: "down" }
      }
    },
    coupling: { size: [96, 64], ports: {} },
    crusher: {
      size: [64, 64],
      ports: { inlet: { at: [32, 0], direction: "up" }, outlet: { at: [32, 64], direction: "down" } }
    },
    cyclone: {
      size: [64, 64],
      ports: {
        inlet: { at: [0, 14], direction: "left" },
        "gas-outlet": { at: [32, 0], direction: "up" },
        "solids-outlet": { at: [32, 64], direction: "down" }
      }
    },
    cylinder: { size: [96, 40], ports: {} },
    damper: {
      size: [64, 64],
      ports: { inlet: { at: [0, 32], direction: "left" }, outlet: { at: [64, 32], direction: "right" } }
    },
    device: { size: [64, 64], ports: {} },
    "digital-switch": { size: [48, 48], label: "beside", ports: {} },
    "dosing-pump": {
      size: [64, 64],
      ports: { inlet: { at: [14, 64], direction: "down" }, outlet: { at: [14, 0], direction: "up" } }
    },
    drive: { size: [64, 64], ports: {} },
    drum: { size: [64, 64], ports: {} },
    encoder: { size: [64, 64], ports: {} },
    fan: { size: [64, 64], ports: {} },
    filter: {
      size: [64, 64],
      ports: { inlet: { at: [0, 32], direction: "left" }, outlet: { at: [64, 32], direction: "right" } }
    },
    "flow-sensor": { size: [48, 48], label: "beside", ports: {} },
    "gravity-tank": {
      size: [64, 64],
      ports: { inlet: { at: [20, 0], direction: "up" }, outlet: { at: [32, 64], direction: "down" } }
    },
    "heat-draw-off": {
      size: [64, 64],
      ports: { inlet: { at: [0, 44], direction: "left" }, outlet: { at: [64, 44], direction: "right" } }
    },
    "heat-exchanger": {
      size: [64, 64],
      ports: {
        inlet: { at: [0, 32], direction: "left" },
        outlet: { at: [64, 32], direction: "right" },
        "medium-inlet": { at: [32, 0], direction: "up" },
        "medium-outlet": { at: [32, 64], direction: "down" }
      }
    },
    heater: {
      size: [64, 64],
      ports: { inlet: { at: [20, 0], direction: "up" }, outlet: { at: [64, 16], direction: "right" } }
    },
    "jacketed-vessel": {
      size: [64, 64],
      ports: {
        inlet: { at: [32, 0], direction: "up" },
        outlet: { at: [32, 64], direction: "down" },
        "jacket-inlet": { at: [0, 52], direction: "left" },
        "jacket-outlet": { at: [64, 26], direction: "right" }
      }
    },
    "level-sensor": { size: [48, 48], label: "beside", ports: {} },
    "level-switch": { size: [48, 48], label: "beside", ports: {} },
    mixer: {
      size: [64, 64],
      ports: {
        inlet: { at: [0, 32], direction: "left" },
        "inlet-2": { at: [24, 64], direction: "down" },
        outlet: { at: [64, 32], direction: "right" }
      }
    },
    motor: { size: [64, 64], ports: {} },
    "motor-valve": {
      size: [64, 64],
      ports: { inlet: { at: [0, 44], direction: "left" }, outlet: { at: [64, 44], direction: "right" } }
    },
    "network-link": { size: [64, 64], ports: {} },
    "position-sensor": { size: [48, 48], label: "beside", ports: {} },
    "pressure-sensor": { size: [48, 48], label: "beside", ports: {} },
    "pressure-switch": { size: [48, 48], label: "beside", ports: {} },
    pump: {
      size: [64, 64],
      ports: { inlet: { at: [0, 36], direction: "left" }, outlet: { at: [64, 20], direction: "right" } }
    },
    "refrigeration-unit": { size: [96, 64], ports: {} },
    "rotary-arm": { size: [64, 64], ports: {} },
    screen: {
      size: [64, 64],
      ports: {
        inlet: { at: [8, 0], direction: "up" },
        oversize: { at: [64, 44], direction: "right" },
        undersize: { at: [32, 64], direction: "down" }
      }
    },
    "sensor-pair": { size: [112, 48], label: "beside", ports: {} },
    "servo-motor": { size: [96, 64], ports: {} },
    shaft: { size: [64, 64], ports: {} },
    "speed-sensor": { size: [48, 48], label: "beside", ports: {} },
    supply: { size: [64, 64], ports: { outlet: { at: [64, 32], direction: "right" } } },
    tank: {
      size: [64, 64],
      ports: { inlet: { at: [20, 0], direction: "up" }, outlet: { at: [64, 46], direction: "right" } }
    },
    "tank-cascade": {
      size: [96, 72],
      ports: { inlet: { at: [14, 0], direction: "up" }, outlet: { at: [96, 60], direction: "right" } }
    },
    "temperature-sensor": { size: [48, 48], label: "beside", ports: {} },
    valve: {
      size: [64, 64],
      ports: { inlet: { at: [0, 44], direction: "left" }, outlet: { at: [64, 44], direction: "right" } }
    },
    vessel: {
      size: [64, 64],
      ports: { inlet: { at: [32, 0], direction: "up" }, outlet: { at: [32, 64], direction: "down" } }
    },
    vfd: { size: [64, 64], ports: {} },
    "weigh-scale": { size: [64, 64], ports: {} }
  };

  // src/hmiBuilder/drawing/symbols/index.ts
  var REGISTERED = /* @__PURE__ */ new Map();
  var TITLES = /* @__PURE__ */ new Map();
  function registerSymbols(models) {
    for (const model of models) {
      REGISTERED.set(`my:${model.name}`, customSymbol(model));
      TITLES.set(`my:${model.name}`, model.title);
    }
  }
  function symbolFor(type) {
    return SYMBOLS[type] ?? REGISTERED.get(type);
  }
  function symbolPlacement(size, symbolSize, label = "under") {
    const [width, height] = size;
    const [symbolWidth, symbolHeight] = symbolSize;
    if (label === "beside") {
      const fit = Math.max(0, Math.min(height / symbolHeight, width / 2 / symbolWidth));
      return { left: 0, top: round((height - symbolHeight * fit) / 2), scale: fit, area: height };
    }
    const area = Math.max(0, height - LABEL_HEIGHT);
    const scale = Math.max(0, Math.min(width / symbolWidth, area / symbolHeight));
    return { left: round((width - symbolWidth * scale) / 2), top: round((area - symbolHeight * scale) / 2), scale, area };
  }
  var ART = {
    pump: PUMP,
    valve: VALVE,
    "control-valve": CONTROL_VALVE,
    tank: TANK,
    "gravity-tank": GRAVITY_TANK,
    "tank-cascade": TANK_CASCADE,
    "dosing-pump": DOSING_PUMP,
    supply: SUPPLY,
    mixer: MIXER,
    "temperature-sensor": instrument("TT"),
    "level-sensor": instrument("LT"),
    "flow-sensor": instrument("FT"),
    "pressure-sensor": instrument("PT"),
    "position-sensor": instrument("ZT"),
    "speed-sensor": instrument("ST"),
    "sensor-pair": SENSOR_PAIR,
    controller: CONTROLLER,
    "cold-room": COLD_ROOM,
    "refrigeration-unit": REFRIGERATION_UNIT,
    heater: HEATER,
    cooler: COOLER,
    "jacketed-vessel": JACKETED_VESSEL,
    "coupled-vessels": COUPLED_VESSELS,
    "heat-draw-off": HEAT_DRAW_OFF,
    motor: MOTOR,
    device: DEVICE,
    "servo-motor": SERVO_MOTOR,
    shaft: SHAFT,
    drum: DRUM,
    coupling: COUPLING,
    encoder: ENCODER,
    "rotary-arm": ROTARY_ARM,
    "axis-indicator": AXIS_INDICATOR,
    "network-link": NETWORK_LINK,
    fan: FAN,
    blower: BLOWER,
    agitator: AGITATOR,
    conveyor: CONVEYOR,
    damper: DAMPER,
    cylinder: CYLINDER,
    "motor-valve": MOTOR_VALVE,
    "weigh-scale": WEIGH_SCALE,
    analyzer: ANALYZER,
    "level-switch": LEVEL_SWITCH,
    "digital-switch": DIGITAL_SWITCH,
    "pressure-switch": PRESSURE_SWITCH,
    vessel: VESSEL,
    "heat-exchanger": HEAT_EXCHANGER,
    filter: FILTER,
    cyclone: CYCLONE,
    crusher: CRUSHER,
    screen: SCREEN,
    vfd: VFD,
    drive: DRIVE,
    contactor: CONTACTOR,
    breaker: BREAKER
  };
  function shipped(type, art) {
    const geometry = SYMBOL_GEOMETRY[type];
    if (!geometry) throw new Error(`The symbol ${type} has no geometry in the runtime's table.`);
    return { ...art, ...geometry };
  }
  var SYMBOLS = Object.fromEntries(
    Object.entries(ART).map(([type, art]) => [type, shipped(type, art)])
  );

  // src/hmiBuilder/drawing/adaptive.ts
  var SPACE = 16;
  var TITLE = 44;
  var TOUCH = 44;
  var CONTAINERS = /* @__PURE__ */ new Set(["panel", "group"]);
  var CONTROLS = /* @__PURE__ */ new Set([
    "button",
    "pulse-button",
    "switch",
    "selector",
    "mode-selector",
    "slider",
    "numeric-input",
    "setpoint",
    "page-button"
  ]);
  function isGraphic(type) {
    return symbolFor(type) !== void 0 || type === "junction" || type === "line" || type === "image";
  }
  function adaptPage(page, design, area) {
    if (page.regions?.length) return { page, size: design, fit: area[0] >= design[0] && area[1] >= design[1] };
    if (area[0] >= design[0] && area[1] >= design[1]) return { page, size: design, fit: true };
    if (area[0] >= design[0]) return { page, size: design, fit: false };
    const width = area[0];
    const blocks = blocksOf(page);
    if (blocks.length === 0) return { page, size: [width, area[1]], fit: false };
    const margin = Math.max(0, Math.min(...blocks.map((block) => block.box.x)));
    const top = Math.max(0, Math.min(...blocks.map((block) => block.box.y)));
    const placed3 = [];
    const objects = /* @__PURE__ */ new Map();
    let pipePoint;
    for (const block of readingOrder(blocks)) {
      const laid = layOut(block, width - 2 * margin);
      let x = block.box.x;
      if (x + laid.width > width - margin) x = Math.max(margin, width - margin - laid.width);
      const box = { x, y: block.box.y, width: laid.width, height: laid.height };
      if (placed3.some((other) => overlap(box, other))) box.x = margin;
      for (let blocking = placed3.filter((other) => overlap(box, other)); blocking.length > 0; blocking = placed3.filter((other) => overlap(box, other))) {
        box.y = Math.max(...blocking.map((other) => other.y + other.height)) + SPACE;
      }
      placed3.push(box);
      for (const [id, where] of laid.objects)
        objects.set(id, { at: [box.x + where.at[0], box.y + where.at[1]], size: where.size });
      if (block.graphic) pipePoint = (point2) => add(laid.point(point2), [box.x, box.y]);
    }
    const right = Math.max(...placed3.map((box) => box.x + box.width));
    const bottom = Math.max(...placed3.map((box) => box.y + box.height));
    return {
      page: {
        ...page,
        object: page.object.map((object) => {
          const where = objects.get(object.id);
          return where ? { ...object, at: where.at, size: where.size } : object;
        }),
        pipe: page.pipe.map(
          (pipe) => pipe.waypoints && pipePoint ? { ...pipe, waypoints: pipe.waypoints.map(pipePoint) } : pipe
        )
      },
      size: [Math.max(width, right + margin), Math.max(area[1], bottom + top)],
      fit: false
    };
  }
  function readingOrder(blocks) {
    const bands = [];
    let bandBottom = -Infinity;
    for (const block of [...blocks].sort((a, b) => a.box.y - b.box.y || a.box.x - b.box.x)) {
      const bottom = block.box.y + block.box.height;
      if (bands.length > 0 && block.box.y < bandBottom) {
        bands[bands.length - 1].push(block);
        bandBottom = Math.max(bandBottom, bottom);
      } else {
        bands.push([block]);
        bandBottom = bottom;
      }
    }
    return bands.flatMap((band) => {
      const columns = [];
      for (const block of [...band].sort((a, b) => a.box.x - b.box.x || a.box.y - b.box.y)) {
        const last = columns[columns.length - 1];
        if (last && block.box.x < last.right) {
          last.blocks.push(block);
          last.right = Math.max(last.right, block.box.x + block.box.width);
        } else {
          columns.push({ right: block.box.x + block.box.width, blocks: [block] });
        }
      }
      return columns.flatMap((column) => column.blocks.sort((a, b) => a.box.y - b.box.y || a.box.x - b.box.x));
    });
  }
  function blocksOf(page) {
    const containers = page.object.filter((object) => CONTAINERS.has(object.type));
    const holder = /* @__PURE__ */ new Map();
    for (const object of page.object) {
      const holders = containers.filter((container) => container.id !== object.id && holds(container, object));
      holders.sort((a, b) => a.size[0] * a.size[1] - b.size[0] * b.size[1]);
      if (holders[0]) holder.set(object.id, holders[0].id);
    }
    const itemOf = (object) => ({
      object,
      inside: page.object.filter((other) => holder.get(other.id) === object.id).map(itemOf)
    });
    const loose = page.object.filter((object) => !holder.has(object.id));
    const blocks = loose.filter((object) => CONTAINERS.has(object.type)).map((object) => ({ container: itemOf(object), items: [], box: boxOf(object) }));
    const graphic = loose.filter((object) => !CONTAINERS.has(object.type) && isGraphic(object.type));
    for (const object of loose.filter((candidate) => !CONTAINERS.has(candidate.type) && !isGraphic(candidate.type))) {
      blocks.push({ items: [itemOf(object)], box: boxOf(object) });
    }
    if (graphic.length > 0) {
      const points = [
        ...graphic.flatMap((object) => [object.at, add(object.at, object.size)]),
        ...page.pipe.flatMap((pipe) => pipe.waypoints ?? [])
      ];
      const [left, upper] = [Math.min(...points.map((point2) => point2[0])), Math.min(...points.map((point2) => point2[1]))];
      const box = {
        x: left,
        y: upper,
        width: Math.max(...points.map((point2) => point2[0])) - left,
        height: Math.max(...points.map((point2) => point2[1])) - upper
      };
      blocks.push({ items: graphic.map(itemOf), box, graphic: true });
    }
    return blocks;
  }
  function layOut(block, inner) {
    const items = block.container ? [block.container] : block.items;
    const all = items.flatMap(flatten);
    if (block.box.width <= inner) return scaled(all, block.box, 1);
    const container = block.container?.object;
    if (block.container && container?.arrange) return arranged(block.container, inner);
    return scaled(all, block.box, scaleFor(all, inner / block.box.width));
  }
  function scaled(objects, box, k) {
    const point2 = (point3) => [
      Math.round((point3[0] - box.x) * k),
      Math.round((point3[1] - box.y) * k)
    ];
    const laid = new Map(
      objects.map((object) => [
        object.id,
        {
          at: point2(object.at),
          size: [Math.round(object.size[0] * k), Math.round(object.size[1] * k)]
        }
      ])
    );
    return { width: Math.round(box.width * k), height: Math.round(box.height * k), objects: laid, point: point2 };
  }
  function scaleFor(objects, wanted) {
    const least = Math.max(
      0,
      ...objects.filter((object) => CONTROLS.has(object.type)).map((object) => Math.min(1, Math.max(TOUCH / object.size[0], TOUCH / object.size[1])))
    );
    return Math.min(1, Math.max(wanted, least));
  }
  function arranged(container, width) {
    const object = container.object;
    const gap = object.gap ?? SPACE;
    const upper = object.label ? TITLE : SPACE;
    const room = width - 2 * SPACE;
    const items = [...container.inside].sort(
      (a, b) => a.object.at[1] - b.object.at[1] || a.object.at[0] - b.object.at[0]
    );
    const pieces = items.map((item) => {
      const box = boxOf(item.object);
      const all = flatten(item);
      return scaled(all, box, box.width > room ? scaleFor(all, room / box.width) : 1);
    });
    const corners = [];
    let height = 0;
    if (object.arrange === "grid") {
      const cell2 = [
        Math.max(0, ...pieces.map((piece) => piece.width)),
        Math.max(0, ...pieces.map((piece) => piece.height))
      ];
      const columns = Math.max(1, Math.floor((room + gap) / (cell2[0] + gap)));
      pieces.forEach(
        (_piece, index) => corners.push([index % columns * (cell2[0] + gap), Math.floor(index / columns) * (cell2[1] + gap)])
      );
      height = pieces.length > 0 ? Math.ceil(pieces.length / columns) * (cell2[1] + gap) - gap : 0;
    } else if (object.arrange === "column") {
      for (const piece of pieces) {
        corners.push([0, height]);
        height += piece.height + gap;
      }
      height = Math.max(0, height - gap);
    } else {
      let [x, y, line] = [0, 0, 0];
      for (const piece of pieces) {
        if (x > 0 && x + piece.width > room) [x, y, line] = [0, y + line + gap, 0];
        corners.push([x, y]);
        x += piece.width + gap;
        line = Math.max(line, piece.height);
      }
      height = y + line;
    }
    const objects = /* @__PURE__ */ new Map();
    const total = upper + height + SPACE;
    objects.set(object.id, { at: [0, 0], size: [width, total] });
    pieces.forEach((piece, index) => {
      for (const [id, where] of piece.objects)
        objects.set(id, { at: add(where.at, [SPACE + corners[index][0], upper + corners[index][1]]), size: where.size });
    });
    const origin = boxOf(object);
    return { width, height: total, objects, point: (point2) => [point2[0] - origin.x, point2[1] - origin.y] };
  }
  function flatten(item) {
    return [item.object, ...item.inside.flatMap(flatten)];
  }
  function boxOf(object) {
    return { x: object.at[0], y: object.at[1], width: object.size[0], height: object.size[1] };
  }
  function holds(container, object) {
    const [outer, inner] = [boxOf(container), boxOf(object)];
    return inner.x >= outer.x && inner.y >= outer.y && inner.x + inner.width <= outer.x + outer.width && inner.y + inner.height <= outer.y + outer.height && inner.width * inner.height < outer.width * outer.height;
  }
  function overlap(a, b) {
    return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
  }
  function add(a, b) {
    return [a[0] + b[0], a[1] + b[1]];
  }

  // src/hmiBuilder/drawing/alarms.ts
  var COLOURS = { high: "alarm-high", medium: "alarm-medium", low: "alarm-low" };
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
  function stateName(state5) {
    const word = STATES[state5];
    return word ? say(word) : state5;
  }
  var HIDDEN = { shelved: "Shelved", suppressed: "Suppressed", out_of_service: "Out of service" };
  function rowStateName(row, tab) {
    const hidden = HIDDEN[tab];
    return hidden ? say(row.active ? "{state}, condition active" : "{state}, condition clear", { state: say(hidden) }) : stateName(row.state);
  }
  function markShapes(priority, x, y, size) {
    const half = size / 2;
    const fill2 = token(COLOURS[priority]);
    const shape = priority === "high" ? el("path", { d: `M${x} ${y - half}L${x + half} ${y}L${x} ${y + half}L${x - half} ${y}Z`, fill: fill2 }) : priority === "medium" ? el("path", {
      d: `M${x} ${y - half}L${x + half} ${y + half * 0.8}L${x - half} ${y + half * 0.8}Z`,
      fill: fill2,
      "stroke-linejoin": "round"
    }) : el("circle", { cx: x, cy: y, r: half * 0.84, fill: fill2 });
    const drop = priority === "medium" ? size * 0.3 : size * 0.2;
    return [
      shape,
      text("!", {
        x,
        y: y + drop,
        fill: token("page"),
        "font-size": Math.round(size * 0.58),
        "font-weight": 800,
        "text-anchor": "middle"
      })
    ];
  }
  function priorityMark(priority, x, y, size) {
    return el("g", { "data-part": "priority-mark", "data-priority": priority }, markShapes(priority, x, y, size));
  }
  function priorityMarkImage(value, size) {
    const priority = alarmPriority(value);
    return el(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        width: size,
        height: size,
        viewBox: `0 0 ${size} ${size}`,
        role: "img",
        "aria-label": say("{priority} priority", { priority: say(NAMES[priority]) }),
        "data-part": "priority-mark",
        "data-priority": priority
      },
      markShapes(priority, size / 2, size / 2, size)
    );
  }
  function blinking(node, nowMs) {
    node.attrs["data-blink"] = "true";
    node.attrs.class = "hmi-blink";
    node.attrs.style = `animation-delay: -${Math.floor(nowMs % 1e3)}ms`;
    return node;
  }
  var ORDER = ["high", "medium", "low"];
  function equipmentAlarms(object, frame) {
    const paths = Object.values(object.bind).map((binding) => binding.path.toLowerCase());
    return (frame.alarms?.summary?.active ?? []).filter((row) => {
      const equipment = row.equipment?.toLowerCase();
      return equipment !== void 0 && paths.some((path) => path.startsWith(`${equipment}.`));
    });
  }
  var equipmentCountWidth = (count) => 12 + String(count).length * 9;
  function equipmentBadgeWidth(object, frame) {
    const count = equipmentAlarms(object, frame).length;
    return count === 0 ? 0 : 30 + (count > 1 ? 4 + equipmentCountWidth(count) : 0);
  }
  function equipmentBadge(object, frame) {
    const rows = equipmentAlarms(object, frame);
    if (rows.length === 0) return void 0;
    const top = [...rows].sort(
      (a, b) => ORDER.indexOf(alarmPriority(a.priority)) - ORDER.indexOf(alarmPriority(b.priority)) || b.since_ms - a.since_ms
    )[0];
    const cx = object.size[0] - 14;
    const cy = 14;
    const mark = priorityMark(alarmPriority(top.priority), cx, cy, 22);
    if (frame.alarms?.stale !== true && rows.some((row) => row.unacknowledged)) blinking(mark, frame.nowMs ?? 0);
    const children = [el("circle", { cx, cy, r: 15, fill: token("surface"), stroke: token("border") }), mark];
    if (rows.length > 1) {
      const count = String(rows.length);
      const width = equipmentCountWidth(rows.length);
      const left = cx - 15 - 4 - width;
      children.unshift(
        el("rect", { x: left, y: cy - 11, width, height: 22, rx: 11, fill: token("raised"), stroke: token("border") }),
        text(count, {
          x: left + width / 2,
          y: cy + 5,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 700,
          "text-anchor": "middle"
        })
      );
    }
    return el("g", { "data-part": "equipment-alarm", "data-alarm": top.path, cursor: "pointer" }, children);
  }

  // src/hmiBuilder/drawing/outline.ts
  var CURVE_PIECES = 16;
  var ARC_STEP = Math.PI / 32;
  var NUMBERS = { M: 2, L: 2, H: 1, V: 1, Q: 4, C: 6, A: 7 };
  var flattened = /* @__PURE__ */ new Map();
  function outlinePolygons(d) {
    const known = flattened.get(d);
    if (known) return known;
    const polygons = flatten2(d);
    flattened.set(d, polygons);
    return polygons;
  }
  function flatten2(d) {
    const tokens = d.match(/[A-Za-z]|[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?/g) ?? [];
    const polygons = [];
    let current2 = [];
    let at = [0, 0];
    let command = "";
    let index = 0;
    const close = () => {
      if (current2.length > 2) polygons.push(current2);
      if (current2.length > 0) at = current2[0];
      current2 = [];
    };
    const numbers = (count) => {
      const taken = tokens.slice(index, index + count).map(Number);
      if (taken.length < count || taken.some((value) => !Number.isFinite(value))) return void 0;
      index += count;
      return taken;
    };
    while (index < tokens.length) {
      if (/^[A-Za-z]$/.test(tokens[index])) command = tokens[index++];
      if (command === "Z") {
        close();
        command = "";
        continue;
      }
      const count = NUMBERS[command];
      const values = count === void 0 ? void 0 : numbers(count);
      if (!values) break;
      switch (command) {
        case "M":
          close();
          at = [values[0], values[1]];
          current2 = [at];
          command = "L";
          break;
        case "L":
          at = [values[0], values[1]];
          current2.push(at);
          break;
        case "H":
          at = [values[0], at[1]];
          current2.push(at);
          break;
        case "V":
          at = [at[0], values[0]];
          current2.push(at);
          break;
        case "Q": {
          const end2 = [values[2], values[3]];
          current2.push(...curve(at, [values[0], values[1]], void 0, end2));
          at = end2;
          break;
        }
        case "C": {
          const end2 = [values[4], values[5]];
          current2.push(...curve(at, [values[0], values[1]], [values[2], values[3]], end2));
          at = end2;
          break;
        }
        case "A": {
          const end2 = [values[5], values[6]];
          current2.push(...arc(at, values[0], values[1], values[2], values[3] !== 0, values[4] !== 0, end2));
          at = end2;
          break;
        }
      }
    }
    close();
    return polygons;
  }
  function curve(from, first, second, to) {
    const points = [];
    for (let piece = 1; piece <= CURVE_PIECES; piece += 1) {
      const t = piece / CURVE_PIECES;
      const u = 1 - t;
      const weights = second ? [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t] : [u * u, 2 * u * t, t * t];
      const controls = second ? [from, first, second, to] : [from, first, to];
      points.push([
        controls.reduce((sum, point2, which) => sum + point2[0] * weights[which], 0),
        controls.reduce((sum, point2, which) => sum + point2[1] * weights[which], 0)
      ]);
    }
    return points;
  }
  function arc(from, rx, ry, rotationDegrees, large, sweep, to) {
    if (rx === 0 || ry === 0 || from[0] === to[0] && from[1] === to[1]) return [to];
    let [radiusX, radiusY] = [Math.abs(rx), Math.abs(ry)];
    const phi = rotationDegrees * Math.PI / 180;
    const [cos, sin] = [Math.cos(phi), Math.sin(phi)];
    const [dx, dy] = [(from[0] - to[0]) / 2, (from[1] - to[1]) / 2];
    const x1 = cos * dx + sin * dy;
    const y1 = -sin * dx + cos * dy;
    const reach = x1 * x1 / (radiusX * radiusX) + y1 * y1 / (radiusY * radiusY);
    if (reach > 1) {
      radiusX *= Math.sqrt(reach);
      radiusY *= Math.sqrt(reach);
    }
    const numerator = radiusX * radiusX * radiusY * radiusY - radiusX * radiusX * y1 * y1 - radiusY * radiusY * x1 * x1;
    const denominator = radiusX * radiusX * y1 * y1 + radiusY * radiusY * x1 * x1;
    const factor = (large !== sweep ? 1 : -1) * Math.sqrt(Math.max(0, numerator / denominator));
    const [cx1, cy1] = [factor * radiusX * y1 / radiusY, -factor * radiusY * x1 / radiusX];
    const centre = [cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2, sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2];
    const angle = (ux, uy, vx, vy) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    const start = angle(1, 0, (x1 - cx1) / radiusX, (y1 - cy1) / radiusY);
    let turn2 = angle((x1 - cx1) / radiusX, (y1 - cy1) / radiusY, (-x1 - cx1) / radiusX, (-y1 - cy1) / radiusY);
    if (!sweep && turn2 > 0) turn2 -= 2 * Math.PI;
    if (sweep && turn2 < 0) turn2 += 2 * Math.PI;
    const pieces = Math.max(4, Math.ceil(Math.abs(turn2) / ARC_STEP));
    const points = [];
    for (let piece = 1; piece < pieces; piece += 1) {
      const theta = start + turn2 * piece / pieces;
      const [ex, ey] = [radiusX * Math.cos(theta), radiusY * Math.sin(theta)];
      points.push([centre[0] + cos * ex - sin * ey, centre[1] + sin * ex + cos * ey]);
    }
    points.push(to);
    return points;
  }

  // src/hmiBuilder/drawing/pipes/ports.ts
  var JUNCTION_PORTS = { left: "left", right: "right", top: "up", bottom: "down" };
  var TURNS = ["right", "down", "left", "up"];
  function portPoint(object, port) {
    const [width, height] = object.size;
    let local;
    if (object.type === "junction") {
      const direction = JUNCTION_PORTS[port];
      local = direction ? { at: [width / 2, height / 2], direction } : void 0;
    } else {
      const definition = symbolFor(object.type);
      const symbolPort = definition?.ports[port];
      if (definition && symbolPort) {
        const { left, top, scale } = symbolPlacement(object.size, definition.size);
        local = {
          at: [left + symbolPort.at[0] * scale, top + symbolPort.at[1] * scale],
          direction: symbolPort.direction
        };
      }
    }
    if (!local) return void 0;
    const turned2 = turn(local, object.rotation ?? 0, width, height);
    return { at: [object.at[0] + turned2.at[0], object.at[1] + turned2.at[1]], direction: turned2.direction };
  }
  function turn(point2, rotation, width, height) {
    const quarters = (Math.round(rotation / 90) % 4 + 4) % 4;
    if (quarters === 0) return point2;
    const [cx, cy] = [width / 2, height / 2];
    let [x, y] = [point2.at[0] - cx, point2.at[1] - cy];
    for (let step = 0; step < quarters; step += 1) [x, y] = [-y, x];
    const direction = TURNS[(TURNS.indexOf(point2.direction) + quarters) % 4];
    return { at: [cx + x, cy + y], direction };
  }

  // src/hmiBuilder/drawing/pipes/route.ts
  var STUB = 16;
  var CLEARANCE = 8;
  var BEND = 40;
  var CACHE_SIZE = 256;
  var STEPS = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };
  var OPPOSITE = { left: "right", right: "left", up: "down", down: "up" };
  var DIRECTIONS = ["left", "right", "up", "down"];
  var cache = /* @__PURE__ */ new Map();
  function routePipe(start, end2, obstacles, waypoints, bounds) {
    const key = JSON.stringify([start, end2, obstacles, waypoints, bounds]);
    const known = cache.get(key);
    if (known) return known;
    const route = simplified(findRoute(start, end2, obstacles, waypoints, bounds));
    if (cache.size >= CACHE_SIZE) cache.delete(cache.keys().next().value);
    cache.set(key, route);
    return route;
  }
  function findRoute(start, end2, obstacles, waypoints, bounds) {
    const boxes = obstacles.map((box) => ({
      x: box.x - CLEARANCE,
      y: box.y - CLEARANCE,
      width: box.width + 2 * CLEARANCE,
      height: box.height + 2 * CLEARANCE
    }));
    if (waypoints.length === 0 && facing(start, end2) && !boxes.some((box) => crosses([start.at, end2.at], box)))
      return [start.at, end2.at];
    const exit = moved(start.at, start.direction, STUB);
    const entry = moved(end2.at, end2.direction, STUB);
    const solid = boxes.filter((box) => ![exit, entry].some((point2) => inside(point2, box)));
    const stops = [exit, ...waypoints, entry];
    const grid = new Grid(stops, solid, bounds);
    const path = [start.at];
    let heading = start.direction;
    for (let leg = 0; leg < stops.length - 1; leg += 1) {
      const last = leg === stops.length - 2;
      const found = grid.search(stops[leg], stops[leg + 1], heading, last ? OPPOSITE[end2.direction] : void 0);
      if (!found) return [start.at, exit, [entry[0], exit[1]], entry, end2.at];
      path.push(...found.points);
      heading = found.heading;
    }
    path.push(end2.at);
    return path;
  }
  function facing(start, end2) {
    const [dx, dy] = [end2.at[0] - start.at[0], end2.at[1] - start.at[1]];
    if (start.direction !== OPPOSITE[end2.direction]) return false;
    const [sx, sy] = STEPS[start.direction];
    return sx !== 0 && dy === 0 && Math.sign(dx) === sx || sy !== 0 && dx === 0 && Math.sign(dy) === sy;
  }
  function moved(point2, direction, distance2) {
    const [sx, sy] = STEPS[direction];
    return [point2[0] + sx * distance2, point2[1] + sy * distance2];
  }
  function inside(point2, box) {
    return point2[0] > box.x && point2[0] < box.x + box.width && point2[1] > box.y && point2[1] < box.y + box.height;
  }
  function crosses([a, b], box) {
    const [x1, x2] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
    const [y1, y2] = [Math.min(a[1], b[1]), Math.max(a[1], b[1])];
    return x1 < box.x + box.width && x2 > box.x && y1 < box.y + box.height && y2 > box.y;
  }
  function simplified(points) {
    const kept = [];
    for (const point2 of points) {
      const last = kept[kept.length - 1];
      if (last && last[0] === point2[0] && last[1] === point2[1]) continue;
      const before = kept[kept.length - 2];
      if (last && before && (before[0] === last[0] && last[0] === point2[0] || before[1] === last[1] && last[1] === point2[1]))
        kept.pop();
      kept.push(point2);
    }
    return kept;
  }
  var Grid = class {
    constructor(stops, boxes, bounds) {
      this.boxes = boxes;
      const within = (value, limit) => value >= 0 && value <= limit;
      this.xs = unique([
        ...stops.map((point2) => point2[0]),
        ...boxes.flatMap((box) => [box.x, box.x + box.width]).filter((x) => within(x, bounds[0]))
      ]);
      this.ys = unique([
        ...stops.map((point2) => point2[1]),
        ...boxes.flatMap((box) => [box.y, box.y + box.height]).filter((y) => within(y, bounds[1]))
      ]);
    }
    /** The cheapest way from one stop to the next, starting with a heading, with a bend's cost
     * when it does not arrive with the heading asked for. */
    search(from, to, heading, arriving) {
      const [width, height] = [this.xs.length, this.ys.length];
      const node = (point2) => this.xs.indexOf(rounded(point2[0])) + this.ys.indexOf(rounded(point2[1])) * width;
      const start = node(from);
      const goal = node(to);
      const goalPoint = to;
      const state5 = (cell2, direction) => cell2 * 4 + direction;
      const best = /* @__PURE__ */ new Map();
      const parent = /* @__PURE__ */ new Map();
      const open = new Heap();
      const first = state5(start, DIRECTIONS.indexOf(heading));
      best.set(first, 0);
      open.push(first, distance(from, goalPoint));
      while (open.size > 0) {
        const current2 = open.pop();
        const cell2 = Math.floor(current2 / 4);
        const direction = current2 % 4;
        const cost = best.get(current2) ?? Infinity;
        if (cell2 === goal) {
          const points = [];
          for (let at = current2; at !== void 0; at = parent.get(at))
            points.unshift(this.point(Math.floor(at / 4)));
          return { points, heading: DIRECTIONS[direction] };
        }
        const [i, j] = [cell2 % width, Math.floor(cell2 / width)];
        DIRECTIONS.forEach((next, nextIndex) => {
          const [sx, sy] = STEPS[next];
          const [ni, nj] = [i + sx, j + sy];
          if (ni < 0 || nj < 0 || ni >= width || nj >= height) return;
          const target = this.point(ni + nj * width);
          const here = this.point(cell2);
          if (!this.open([here, target])) return;
          const bends = (next === DIRECTIONS[direction] ? 0 : BEND) + (ni + nj * width === goal && arriving !== void 0 && next !== arriving ? BEND : 0);
          if (next === OPPOSITE[DIRECTIONS[direction]]) return;
          const nextState = state5(ni + nj * width, nextIndex);
          const nextCost = cost + distance(here, target) + bends;
          if (nextCost < (best.get(nextState) ?? Infinity)) {
            best.set(nextState, nextCost);
            parent.set(nextState, current2);
            open.push(nextState, nextCost + distance(target, goalPoint));
          }
        });
      }
      return void 0;
    }
    point(cell2) {
      const width = this.xs.length;
      return [this.xs[cell2 % width], this.ys[Math.floor(cell2 / width)]];
    }
    /** Whether a step between two neighbouring grid points stays out of every object. */
    open([a, b]) {
      const middle2 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
      return !this.boxes.some((box) => inside(middle2, box) || inside(b, box));
    }
  };
  function rounded(value) {
    return Math.round(value * 100) / 100;
  }
  function unique(values) {
    return [...new Set(values.map(rounded))].sort((a, b) => a - b);
  }
  function distance(a, b) {
    return Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]);
  }
  var Heap = class {
    constructor() {
      this.items = [];
      this.count = 0;
    }
    get size() {
      return this.items.length;
    }
    push(state5, priority) {
      const items = this.items;
      items.push({ state: state5, priority, order: this.count++ });
      let at = items.length - 1;
      while (at > 0) {
        const up = at - 1 >> 1;
        if (!this.before(items[at], items[up])) break;
        [items[at], items[up]] = [items[up], items[at]];
        at = up;
      }
    }
    pop() {
      const items = this.items;
      const top = items[0];
      const last = items.pop();
      if (items.length > 0) {
        items[0] = last;
        let at = 0;
        for (; ; ) {
          const [left, right] = [at * 2 + 1, at * 2 + 2];
          let smallest = at;
          if (left < items.length && this.before(items[left], items[smallest])) smallest = left;
          if (right < items.length && this.before(items[right], items[smallest])) smallest = right;
          if (smallest === at) break;
          [items[at], items[smallest]] = [items[smallest], items[at]];
          at = smallest;
        }
      }
      return top.state;
    }
    before(a, b) {
      return a.priority < b.priority || a.priority === b.priority && a.order < b.order;
    }
  };

  // src/hmiBuilder/drawing/pipes/draw.ts
  function pipeRoutes(page, size) {
    if (page.pipe.length === 0) return [];
    const obstacles = page.object.flatMap(solidBoxes);
    return page.pipe.map((pipe) => {
      const from = end(page, pipe.from);
      const to = end(page, pipe.to);
      return from && to ? routePipe(from, to, obstacles, pipe.waypoints ?? [], size) : void 0;
    });
  }
  function drawPipes(page, frame) {
    const routes = pipeRoutes(page, frame.size);
    return page.pipe.flatMap((pipe, index) => {
      const points = routes[index];
      if (!points) return [];
      const d = `M${points.map(([x, y]) => `${round2(x)} ${round2(y)}`).join(" L")}`;
      const signal = pipe.style === "signal";
      const children = [
        el("path", {
          "data-part": "line",
          d,
          fill: "none",
          stroke: token("muted"),
          "stroke-width": signal ? 1.5 : 4,
          "stroke-linejoin": "round",
          "stroke-linecap": "round",
          "stroke-dasharray": signal ? "6 5" : void 0,
          opacity: signal ? 0.9 : 0.75
        }),
        ...flow(pipe, d, frame)
      ];
      return [
        el("g", { class: "hmi-pipe", "data-pipe": String(index), "data-style": signal ? "signal" : "process" }, children)
      ];
    });
  }
  function flow(pipe, d, frame) {
    const binding = pipe.bind?.flow;
    const live = binding ? frame.values[binding.path] : void 0;
    if (!observed(live)) return [];
    const value = live.value;
    const direction = value === true || typeof value === "number" && value > 0 ? 1 : typeof value === "number" && value < 0 ? -1 : 0;
    if (direction === 0) return [];
    const estimated = live.state === "estimated";
    return [
      el("path", {
        "data-part": "flow",
        "data-estimated": estimated ? "true" : void 0,
        class: direction > 0 ? "hmi-flow" : "hmi-flow-back",
        d,
        fill: "none",
        stroke: token("accent"),
        "stroke-width": 4,
        "stroke-linecap": "round",
        "stroke-dasharray": "1 15",
        opacity: estimated ? 0.5 : void 0,
        style: `animation-delay: -${Math.floor(frame.nowMs ?? 0) % FLOW_PERIOD_MS}ms`
      })
    ];
  }
  function end(page, written) {
    const at = written.lastIndexOf(".");
    if (at <= 0) return void 0;
    const object = page.object.find((candidate) => candidate.id === written.slice(0, at));
    return object ? portPoint(object, written.slice(at + 1)) : void 0;
  }
  function solidBoxes(object) {
    const [x, y] = object.at;
    const [width, height] = object.size;
    if (object.type === "junction") return [];
    const definition = SYMBOLS[object.type];
    if (!definition || definition.label === "beside" || (object.rotation ?? 0) % 360 !== 0)
      return [{ x, y, width, height }];
    const { left, top, scale, area } = symbolPlacement(object.size, definition.size);
    return [
      { x: x + left, y: y + top, width: definition.size[0] * scale, height: definition.size[1] * scale },
      { x, y: y + area, width, height: LABEL_HEIGHT }
    ];
  }
  function round2(value) {
    return Math.round(value * 100) / 100;
  }

  // src/hmiBuilder/drawing/attachments.ts
  function drawAttachments(page, frame) {
    const attached = page.object.filter((object) => object.attach !== void 0);
    if (attached.length === 0) return [];
    const routes = attached.some((object) => object.attach?.startsWith("pipe:")) ? pipeRoutes(page, frame.size) : [];
    return attached.flatMap((object) => {
      const from = attachOrigin(object).centre;
      const path = connection(page, object.attach ?? "", routes, from);
      if (!path) return [];
      return [
        el("polyline", {
          "data-attachment": object.id,
          points: path.map((point2) => point2.join(",")).join(" "),
          fill: "none",
          stroke: token("muted"),
          "stroke-width": 1.5,
          "stroke-linejoin": "round",
          "stroke-linecap": "round"
        })
      ];
    });
  }
  function connection(page, attach, routes, from) {
    const pipe = /^pipe:(\d+)$/.exec(attach);
    if (pipe) {
      const route = routes[Number(pipe[1])];
      const to = route ? nearestOnRoute(route, from) : void 0;
      if (!to) return void 0;
      return to[0] === from[0] || to[1] === from[1] ? [from, to] : [from, [from[0], to[1]], to];
    }
    const object = page.object.find((candidate) => candidate.id === attach);
    if (!object) return void 0;
    const outline2 = drawnOutline(object);
    return outline2 ? toOutline(from, outline2) : toBox(from, symbolBox(object));
  }
  function toOutline(from, outline2) {
    const met = meet(toBox(from, boundsOf(outline2.flat())), outline2);
    if (met) return met;
    const away = (polygon) => distanceToBox(from, boundsOf(polygon));
    const nearest = outline2.reduce((best, polygon) => away(polygon) < away(best) ? polygon : best);
    const route = toBox(from, boundsOf(nearest));
    return meet(route, [nearest]) ?? route;
  }
  function meet(route, outline2) {
    const [start, end2] = route.slice(-2);
    if (start[0] === end2[0] && start[1] === end2[1]) return route;
    const direction = [end2[0] - start[0], end2[1] - start[1]];
    let nearest;
    for (const polygon of outline2) {
      for (let index = 0; index < polygon.length; index += 1) {
        const along = crossing(start, direction, polygon[index], polygon[(index + 1) % polygon.length]);
        if (along !== void 0 && (nearest === void 0 || along < nearest)) nearest = along;
      }
    }
    if (nearest === void 0) return void 0;
    return [...route.slice(0, -1), [round3(start[0] + direction[0] * nearest), round3(start[1] + direction[1] * nearest)]];
  }
  function crossing(start, direction, a, b) {
    const piece = [b[0] - a[0], b[1] - a[1]];
    const across = direction[0] * piece[1] - direction[1] * piece[0];
    if (Math.abs(across) < 1e-12) return void 0;
    const offset = [a[0] - start[0], a[1] - start[1]];
    const along = (offset[0] * piece[1] - offset[1] * piece[0]) / across;
    const share2 = (offset[0] * direction[1] - offset[1] * direction[0]) / across;
    return along >= -1e-9 && share2 >= -1e-9 && share2 <= 1 + 1e-9 ? Math.max(0, along) : void 0;
  }
  function drawnOutline(object) {
    const definition = symbolFor(object.type);
    if (!definition?.outline || object.rotation % 360 !== 0) return void 0;
    const { left, top, scale } = symbolPlacement(object.size, definition.size, definition.label);
    const factor = round3(scale);
    const [x, y] = object.at;
    const polygons = outlinePolygons(definition.outline).map(
      (polygon) => polygon.map(([px, py]) => [x + left + px * factor, y + top + py * factor])
    );
    return polygons.length > 0 ? polygons : void 0;
  }
  function boundsOf(points) {
    const xs = points.map((point2) => point2[0]);
    const ys = points.map((point2) => point2[1]);
    const [x, y] = [Math.min(...xs), Math.min(...ys)];
    return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
  }
  function distanceToBox(point2, box) {
    const dx = Math.max(box.x - point2[0], 0, point2[0] - (box.x + box.width));
    const dy = Math.max(box.y - point2[1], 0, point2[1] - (box.y + box.height));
    return Math.hypot(dx, dy);
  }
  function toBox(from, box) {
    const [x, y] = from;
    const [right, bottom] = [box.x + box.width, box.y + box.height];
    const overX = x >= box.x && x <= right;
    const overY = y >= box.y && y <= bottom;
    if (overX && overY) return [from, from];
    if (overX) return [from, [x, y < box.y ? box.y : bottom]];
    if (overY) return [from, [x < box.x ? box.x : right, y]];
    const middle2 = round3(box.y + box.height / 2);
    return [from, [x, middle2], [x < box.x ? box.x : right, middle2]];
  }
  function symbolBox(object) {
    const [x, y] = object.at;
    const [width, height] = object.size;
    const definition = symbolFor(object.type);
    if (!definition || object.rotation % 360 !== 0) return { x, y, width, height };
    const { left, top, scale } = symbolPlacement(object.size, definition.size, definition.label);
    const bubble2 = definition.label === "beside" ? definition.size[1] : definition.size[0];
    return { x: x + left, y: y + top, width: round3(bubble2 * scale), height: round3(definition.size[1] * scale) };
  }
  function attachOrigin(object) {
    const box = symbolBox(object);
    return { centre: middle(box), radius: round3(Math.min(box.width, box.height) / 2) };
  }
  function middle(box) {
    return [round3(box.x + box.width / 2), round3(box.y + box.height / 2)];
  }
  function nearestOnRoute(route, point2) {
    let best;
    let distance2 = Infinity;
    for (let index = 1; index < route.length; index += 1) {
      const [a, b] = [route[index - 1], route[index]];
      const candidate = [
        clamp(point2[0], Math.min(a[0], b[0]), Math.max(a[0], b[0])),
        clamp(point2[1], Math.min(a[1], b[1]), Math.max(a[1], b[1]))
      ];
      const away = Math.hypot(candidate[0] - point2[0], candidate[1] - point2[1]);
      if (away < distance2) {
        best = candidate;
        distance2 = away;
      }
    }
    return best;
  }
  function clamp(value, low, high) {
    return Math.min(high, Math.max(low, value));
  }
  function round3(value) {
    return Math.round(value * 100) / 100;
  }

  // src/hmiBuilder/drawing/signals.ts
  var BACKGROUNDS = /* @__PURE__ */ new Set(["panel", "group"]);
  function drawSignals(page, frame) {
    const byId = new Map(page.object.map((object) => [object.id, object]));
    return page.object.flatMap((controller) => {
      const lines = [];
      const measured = controller.measures ? byId.get(controller.measures) : void 0;
      const driven = controller.acts_on ? byId.get(controller.acts_on) : void 0;
      const outputSide = driven ? controllerSide(controller, driven) : void 0;
      if (measured) {
        const side = controllerSide(controller, measured, outputSide);
        const route = signalRoute(
          page,
          frame.size,
          bubbleEnd(measured, controller),
          bubbleEnd(controller, measured, side),
          [measured, controller]
        );
        if (route) lines.push(signalLine(`${controller.id}:measures`, route));
      }
      if (driven) {
        const route = signalRoute(
          page,
          frame.size,
          bubbleEnd(controller, driven, outputSide),
          sideEnd(driven, controller),
          [controller, driven]
        );
        if (route) lines.push(signalLine(`${controller.id}:acts_on`, route));
      }
      return lines;
    });
  }
  function signalLine(id, route) {
    return el("polyline", {
      "data-signal": id,
      "data-style": "signal",
      points: route.map(([x, y]) => `${round4(x)},${round4(y)}`).join(" "),
      fill: "none",
      stroke: token("muted"),
      "stroke-width": 1.5,
      "stroke-dasharray": "6 5",
      "stroke-linejoin": "round"
    });
  }
  function signalRoute(page, size, start, end2, ends) {
    const obstacles = page.object.filter((object) => !ends.includes(object) && !BACKGROUNDS.has(object.type)).flatMap(solidBoxes);
    const route = routePipe(start, end2, obstacles, [], size);
    return route.length >= 2 ? route : void 0;
  }
  function controllerSide(controller, other, taken) {
    const box = bubbleBox(controller);
    const target = symbolBox(other);
    const faced = facing2(box, target);
    if (faced !== "right" && faced !== taken) return faced;
    const left = target.x + target.width / 2 < box.x + box.width / 2;
    const below = target.y + target.height / 2 > box.y + box.height / 2;
    const order = left ? ["left", below ? "down" : "up", below ? "up" : "down"] : below ? ["down", "up", "left"] : ["up", "down", "left"];
    return order.find((side) => side !== taken) ?? "left";
  }
  function bubbleBox(object) {
    const { centre, radius } = attachOrigin(object);
    return { x: centre[0] - radius, y: centre[1] - radius, width: 2 * radius, height: 2 * radius };
  }
  function bubbleEnd(object, toward, side) {
    const { centre, radius } = attachOrigin(object);
    const box = bubbleBox(object);
    const direction = side ?? facing2(box, symbolBox(toward));
    const step = STEPS2[direction];
    return { at: [centre[0] + step[0] * radius, centre[1] + step[1] * radius], direction };
  }
  function sideEnd(object, from) {
    const box = symbolBox(object);
    const { centre } = attachOrigin(from);
    const direction = facing2(box, { x: centre[0], y: centre[1], width: 0, height: 0 });
    const [x, y] = [box.x + box.width / 2, box.y + box.height / 2];
    const at = direction === "up" ? [x, box.y] : direction === "down" ? [x, box.y + box.height] : direction === "left" ? [box.x, y] : [box.x + box.width, y];
    return { at, direction };
  }
  var STEPS2 = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };
  function facing2(box, other) {
    if (other.y + other.height <= box.y) return "up";
    if (other.y >= box.y + box.height) return "down";
    return other.x + other.width / 2 < box.x + box.width / 2 ? "left" : "right";
  }
  function round4(value) {
    return Math.round(value * 100) / 100;
  }

  // src/hmiBuilder/drawing/translated.ts
  function translatedPage(page) {
    const object = page.object.map((shown) => ({
      ...shown,
      ...shown.label !== void 0 ? { label: translate(shown.label) } : {},
      ...shown.text !== void 0 ? { text: translate(shown.text) } : {},
      ...shown.confirm !== void 0 ? { confirm: translate(shown.confirm) } : {}
    }));
    return { ...page, title: translate(page.title), object };
  }

  // src/hmiBuilder/drawing/components/listParts.ts
  var PAGER_WIDTH = 204;
  var BUTTON = { width: 40, height: 36 };
  function pager(kind, first, shown, total, right, top) {
    const page = (direction, left, target) => {
      const usable = target !== void 0;
      return el(
        "g",
        {
          "data-part": `${kind}-page`,
          [`data-${kind}-page`]: direction,
          "data-page-first": target,
          "data-disabled": usable ? void 0 : "true",
          cursor: usable ? "pointer" : void 0
        },
        [
          el("rect", {
            x: left,
            y: top,
            width: BUTTON.width,
            height: BUTTON.height,
            rx: BUTTON.height / 2,
            fill: token("raised"),
            stroke: token("border")
          }),
          text(direction === "previous" ? "\u2039" : "\u203A", {
            x: left + BUTTON.width / 2,
            y: top + 24,
            fill: usable ? token("text") : token("unknown"),
            "font-size": 20,
            "font-weight": 700,
            "text-anchor": "middle"
          })
        ]
      );
    };
    const capacity = Math.max(1, shown);
    const previous = first > 0 ? Math.max(0, first - capacity) : void 0;
    const next = first + shown < total ? first + shown : void 0;
    return el("g", { "data-part": `${kind}-pager` }, [
      text(say("{first}\u2013{last} of {total}", { first: first + 1, last: first + shown, total }), {
        x: right - 2 * BUTTON.width - 20,
        y: top + 23,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 550,
        "text-anchor": "end",
        "font-variant-numeric": "tabular-nums"
      }),
      page("previous", right - 2 * BUTTON.width - 8, previous),
      page("next", right - BUTTON.width, next)
    ]);
  }

  // src/hmiBuilder/drawing/components/alarmList.ts
  var TABS = [
    { tab: "active", label: "Active", empty: "No active alarms" },
    { tab: "shelved", label: "Shelved", empty: "No shelved alarms" },
    { tab: "suppressed", label: "Suppressed", empty: "No suppressed alarms" },
    { tab: "out_of_service", label: "Out of service", empty: "No alarms out of service" }
  ];
  var PAD = 16;
  var HEADER = 52;
  var FILTERS = 36;
  var CHIP = 28;
  var HEADINGS = 28;
  var HEADINGS_TOP = HEADER + FILTERS;
  var ROWS_TOP = HEADINGS_TOP + HEADINGS;
  var COLUMNS = [
    { column: "priority", label: "Priority", width: 96 },
    { column: "time", label: "Time", width: 84 },
    { column: "tag", label: "Tag", width: 112 },
    { column: "message", label: "Message", width: 160 },
    { column: "value", label: "Value", width: 152 },
    { column: "state", label: "State", width: 160 }
  ];
  var LEFT_OUT = ["value", "tag", "time", "state"];
  var PRIORITY_RANK = { high: 0, medium: 1, low: 2 };
  var ROW = 50;
  var FOOTER = 48;
  var BUTTON2 = { width: 128, height: 40 };
  function drawAlarmList(object, frame) {
    const [width, height] = object.size;
    const alarms = frame.alarms;
    const view = alarms?.views?.[object.id] ?? {};
    const tab = view.tab ?? "active";
    const stale = alarms?.stale === true;
    const listed = (name) => alarms?.summary?.[name] ?? [];
    const all = listed(tab);
    const rows = sorted(filteredRows(all, view.filter), tab, view.sort);
    const stateWidth = Math.min(
      280,
      Math.max(160, Math.ceil(Math.max(0, ...rows.map((row) => textWidth(rowStateName(row, tab), 14)))) + 16)
    );
    const valueWidth = rows.some((row) => row.raised) ? Math.max(152, ...rows.map((row) => Math.ceil(textWidth(valueText(row, frame.formats), 14, 600)) + 16)) : 0;
    const columns = placed(width, tab === "active", stateWidth, valueWidth);
    const children = [
      el("rect", {
        "data-part": "card",
        x: 0.5,
        y: 0.5,
        width: Math.max(0, width - 1),
        height: Math.max(0, height - 1),
        rx: 14,
        fill: token("surface"),
        stroke: stale ? token("stale") : token("border"),
        "stroke-dasharray": stale ? "6 4" : void 0
      })
    ];
    let x = PAD;
    for (const { tab: name, label } of TABS) {
      const content = `${say(label)} ${listed(name).length}`;
      const tabWidth = Math.round(textWidth(content, 14, 700) + 20);
      const selected = name === tab;
      children.push(
        el(
          "g",
          {
            "data-part": "alarm-tab",
            "data-alarm-tab": name,
            "data-selected": selected ? "true" : void 0,
            cursor: "pointer"
          },
          [
            el("rect", {
              x,
              y: 8,
              width: tabWidth,
              height: 36,
              rx: 18,
              fill: selected ? token("raised") : "transparent",
              stroke: selected ? token("accent") : "none"
            }),
            text(content, {
              x: x + tabWidth / 2,
              y: 31,
              fill: selected ? token("accent") : token("muted"),
              "font-size": 14,
              "font-weight": selected ? 700 : 550,
              "text-anchor": "middle"
            })
          ]
        )
      );
      x += tabWidth + 6;
    }
    const headerFits = x + PAGER_WIDTH <= width - PAD;
    const room = (footer) => Math.max(1, Math.floor((height - ROWS_TOP - (footer ? FOOTER : 8)) / ROW));
    const paged = rows.length > room(false);
    const capacity = paged && !headerFits ? room(true) : room(false);
    const lastPage = Math.max(0, Math.floor((rows.length - 1) / capacity) * capacity);
    const first = Math.min(Math.max(0, view.first ?? 0), lastPage);
    const shown = rows.slice(first, first + capacity);
    if (paged) {
      const top = headerFits ? 8 : height - FOOTER + 4;
      children.push(pager("alarm", first, shown.length, rows.length, width - PAD, top));
    }
    children.push(...filterRow(view.filter, rows.length, all.length, width), ...headings(columns, view.sort));
    if (shown.length === 0) {
      const empty = say(TABS.find((entry) => entry.tab === tab)?.empty ?? "No alarms");
      const top = ROWS_TOP;
      children.push(
        text(empty, {
          x: width / 2,
          y: top + (height - top) / 2,
          fill: token("muted"),
          "font-size": 15,
          "font-weight": 550,
          "text-anchor": "middle"
        })
      );
    }
    shown.forEach((row, index) => {
      children.push(drawRow(row, tab, ROWS_TOP + index * ROW, width, stale, frame.nowMs ?? 0, columns, frame.formats));
    });
    return el("g", { class: "hmi-object hmi-alarm-list", "data-stale": stale ? "true" : void 0 }, children);
  }
  function placed(width, acknowledge, stateWidth, valueWidth) {
    const button = acknowledge ? BUTTON2.width + 12 : 0;
    const needed = (columns2) => 2 * PAD + button + columns2.reduce((sum, entry) => sum + entry.width, 0);
    let columns = COLUMNS.filter((entry) => valueWidth > 0 || entry.column !== "value").map(
      (entry) => entry.column === "state" ? { ...entry, width: stateWidth } : entry.column === "value" ? { ...entry, width: valueWidth } : entry
    );
    for (const column of LEFT_OUT) {
      if (needed(columns) <= width) break;
      columns = columns.filter((entry) => entry.column !== column);
    }
    const spare = Math.max(0, width - needed(columns));
    let x = PAD;
    return columns.map((entry) => {
      const room = entry.column === "message" ? entry.width + spare : entry.width;
      const at = { column: entry.column, label: entry.label, x, width: room };
      x += room;
      return at;
    });
  }
  function headings(columns, sort) {
    return columns.map(({ column, label, x, width }) => {
      const sorted2 = sort?.column === column ? sort.reversed ? "reversed" : "natural" : void 0;
      const words = say(label);
      const right = x + textWidth(words, 14, 600) + 6;
      const arrow = sorted2 === void 0 ? [] : [
        el("path", {
          d: sorted2 === "natural" ? `M${right} ${HEADINGS_TOP + 12} l8 0 l-4 5 z` : `M${right} ${HEADINGS_TOP + 17} l8 0 l-4 -5 z`,
          fill: token("accent")
        })
      ];
      return el(
        "g",
        { "data-part": "alarm-heading", "data-alarm-sort": column, "data-sorted": sorted2, cursor: "pointer" },
        [
          el("rect", { x, y: HEADINGS_TOP, width, height: HEADINGS, fill: "transparent" }),
          text(words, {
            x,
            y: HEADINGS_TOP + 19,
            fill: sorted2 ? token("accent") : token("muted"),
            "font-size": 14,
            "font-weight": 600
          }),
          ...arrow
        ]
      );
    });
  }
  function filtering(filter) {
    return Boolean(
      filter && (filter.text?.trim() || filter.priorities?.length || filter.unacknowledged || filter.equipment)
    );
  }
  function filteredRows(rows, filter) {
    if (!filtering(filter)) return rows;
    const wanted = filter?.text?.trim().toLowerCase();
    return rows.filter(
      (row) => (!filter?.priorities?.length || filter.priorities.includes(alarmPriority(row.priority))) && (!filter?.unacknowledged || row.unacknowledged) && (!filter?.equipment || row.equipment === filter.equipment) && (!wanted || [translate(row.message || row.path), tagOf(row), row.path].some(
        (field) => field.toLowerCase().includes(wanted)
      ))
    );
  }
  function filterRow(filter, shown, total, width) {
    const text_ = filter?.text?.trim();
    const priorities = filter?.priorities ?? [];
    const chips = [
      { kind: "text", label: text_ ? `\u201C${text_}\u201D` : say("Search"), selected: Boolean(text_) },
      ...["high", "medium", "low"].map((priority) => ({
        kind: "priority",
        value: priority,
        label: priorityName(priority),
        selected: priorities.includes(priority)
      })),
      { kind: "unacknowledged", label: say("Unacknowledged"), selected: Boolean(filter?.unacknowledged) },
      {
        kind: "equipment",
        label: filter?.equipment ? filter.equipment.split(".").pop() ?? filter.equipment : say("Equipment"),
        selected: Boolean(filter?.equipment)
      },
      ...filtering(filter) ? [{ kind: "clear", label: say("Clear"), selected: false }] : []
    ];
    const nodes = [];
    const count = filtering(filter) ? say("{count} of {total}", { count: shown, total }) : "";
    const end2 = width - PAD - (count ? textWidth(count, 14) + 12 : 0);
    let x = PAD;
    for (const chip2 of chips) {
      const chipWidth = Math.round(textWidth(chip2.label, 14, 600) + 24);
      if (x + chipWidth > end2) break;
      nodes.push(
        el(
          "g",
          {
            "data-part": "alarm-filter",
            "data-alarm-filter": chip2.kind,
            "data-value": chip2.value,
            "data-selected": chip2.selected ? "true" : void 0,
            cursor: "pointer"
          },
          [
            el("rect", {
              x,
              y: HEADER + 2,
              width: chipWidth,
              height: CHIP,
              rx: CHIP / 2,
              fill: chip2.selected ? token("raised") : "transparent",
              stroke: chip2.selected ? token("accent") : token("border")
            }),
            text(chip2.label, {
              x: x + chipWidth / 2,
              y: HEADER + 21,
              fill: chip2.selected ? token("accent") : token("muted"),
              "font-size": 14,
              "font-weight": 600,
              "text-anchor": "middle"
            })
          ]
        )
      );
      x += chipWidth + 6;
    }
    if (count)
      nodes.push(
        text(count, { x: width - PAD, y: HEADER + 21, fill: token("muted"), "font-size": 14, "text-anchor": "end" })
      );
    return nodes;
  }
  function valueText(row, formats) {
    if (!row.raised) return "";
    const { value, limit, high, source } = row.raised;
    const format = source ? formats?.[formatPath(source)] : void 0;
    const unit = row.raised.unit ?? format?.unit;
    const decimals = (number2) => (String(Number(number2.toFixed(3))).split(".")[1] ?? "").length;
    const places = format?.decimals ?? Math.max(decimals(value), decimals(limit));
    return `${value.toFixed(places)} ${high ? ">" : "<"} ${limit.toFixed(places)}${unit ? ` ${unit}` : ""}`;
  }
  function tagOf(row) {
    return row.equipment?.split(".").pop() ?? "";
  }
  function sorted(rows, tab, sort) {
    if (!sort) return rows;
    const text2 = (row) => {
      if (sort.column === "tag") return tagOf(row);
      if (sort.column === "state") return rowStateName(row, tab);
      return translate(row.message || row.path);
    };
    const compare = (a, b) => {
      if (sort.column === "priority")
        return PRIORITY_RANK[alarmPriority(a.priority)] - PRIORITY_RANK[alarmPriority(b.priority)] || b.since_ms - a.since_ms;
      if (sort.column === "time") return b.since_ms - a.since_ms;
      if (sort.column === "value") return (b.raised?.value ?? -Infinity) - (a.raised?.value ?? -Infinity);
      return text2(a).localeCompare(text2(b));
    };
    return [...rows].sort((a, b) => sort.reversed ? -compare(a, b) : compare(a, b));
  }
  function drawRow(row, tab, top, width, stale, nowMs, columns, formats) {
    const priority = alarmPriority(row.priority);
    const acknowledge = tab === "active" && (row.actions?.includes("acknowledge") ?? false);
    const mark = priorityMark(priority, PAD + 10, top + ROW / 2, 20);
    if (row.unacknowledged && !stale) blinking(mark, nowMs);
    const middle2 = top + ROW / 2 + 5;
    const cell2 = ({ column, x, width: room }) => {
      const bold = row.unacknowledged ? 700 : 550;
      const [content, left, size, weight, colour] = column === "priority" ? [priorityName(priority), x + 28, 14, 600, token("text")] : column === "time" ? [timeOfDay(row.since_ms), x, 14, 400, token("muted")] : column === "tag" ? [tagOf(row), x, 14, 600, token("text")] : column === "message" ? [translate(row.message || row.path), x, 15, bold, token("text")] : column === "value" ? [valueText(row, formats), x, 14, 600, token("text")] : [rowStateName(row, tab), x, 14, 400, token("muted")];
      return el("g", { "data-part": "alarm-cell", "data-column": column }, [
        text(fitWidth(content, Math.max(0, room - (left - x) - 12), size, weight), {
          x: left,
          y: middle2,
          fill: colour,
          "font-size": size,
          "font-weight": weight,
          "font-variant-numeric": column === "time" || column === "value" ? "tabular-nums" : void 0
        })
      ]);
    };
    const children = [
      // The whole row opens the alarm (§5.7, item 4).
      el("rect", { x: 1, y: top, width: Math.max(0, width - 2), height: ROW, fill: "transparent", cursor: "pointer" }),
      el("line", { x1: PAD, y1: top, x2: width - PAD, y2: top, stroke: token("line"), "stroke-width": 1 }),
      mark,
      ...columns.map(cell2)
    ];
    if (acknowledge) {
      const left = width - PAD - BUTTON2.width;
      const buttonTop = top + (ROW - BUTTON2.height) / 2;
      children.push(
        el("g", { "data-alarm-action": "acknowledge", "data-alarm": row.path, cursor: "pointer" }, [
          el("rect", {
            x: left,
            y: buttonTop,
            width: BUTTON2.width,
            height: BUTTON2.height,
            rx: BUTTON2.height / 2,
            fill: token("raised"),
            stroke: token("accent")
          }),
          text(say("Acknowledge"), {
            x: left + BUTTON2.width / 2,
            y: buttonTop + 25,
            fill: token("accent"),
            "font-size": 14,
            "font-weight": 650,
            "text-anchor": "middle"
          })
        ])
      );
    }
    return el("g", { "data-part": "alarm-row", "data-alarm": row.path }, children);
  }

  // src/hmiBuilder/drawing/alarmRanges.ts
  var LIMIT_NAMES = ["low-low", "low", "high", "high-high"];
  function alarmRange(object, live) {
    const limits = live?.presentation ? live.presentation.limits : object.limits;
    if (!limits || !LIMIT_NAMES.some((name) => limits[name] !== void 0)) return void 0;
    const min = object.min ?? 0, max = object.max ?? 100;
    if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) return void 0;
    let previous = -Infinity;
    for (const name of LIMIT_NAMES) {
      const value = limits[name];
      if (value === void 0) continue;
      if (!Number.isFinite(value) || value < previous) return void 0;
      previous = value;
    }
    const fraction = (value) => Math.max(0, Math.min(1, (value - min) / (max - min)));
    return {
      from: fraction(limits.low ?? limits["low-low"] ?? min),
      to: fraction(limits.high ?? limits["high-high"] ?? max),
      ticks: LIMIT_NAMES.flatMap((name) => {
        const value = limits[name];
        return value === void 0 || value < min || value > max ? [] : [{ name, value, fraction: fraction(value), priority: live?.presentation?.priorities?.[name] }];
      })
    };
  }
  function limitTick(tick, coordinates, object) {
    const name = { "low-low": "LL", low: "L", high: "H", "high-high": "HH" }[tick.name];
    const unit = object.unit ?? object.bind.value?.unit;
    const title = `${name} \xB7 ${formatValue(tick.value, object)}${unit ? ` ${unit}` : ""}`;
    return el(
      "line",
      {
        ...coordinates,
        "data-limit": tick.name,
        "data-fraction": tick.fraction,
        stroke: token(
          `alarm-${tick.priority ?? (tick.name === "low-low" || tick.name === "high-high" ? "high" : "medium")}`
        ),
        "stroke-width": 2,
        "stroke-linecap": "round",
        "aria-label": title
      },
      [{ ...el("title"), text: title }]
    );
  }

  // src/hmiBuilder/drawing/ranges.ts
  function rangeReading(live, object) {
    if (!live || live.state === "unknown" || typeof live.value !== "number" || !Number.isFinite(live.value)) {
      return void 0;
    }
    const min = object.min ?? 0;
    const max = object.max ?? 100;
    const raw = max === min ? 0 : (live.value - min) / (max - min);
    if (raw > 1) return { fraction: 1, range: "over" };
    if (raw < 0) return { fraction: 0, range: "under" };
    return { fraction: Number(raw.toFixed(6)) };
  }

  // src/hmiBuilder/drawing/components/bar.ts
  var CAPTION_BELOW = 44;
  var CAPTION_BESIDE = 84;
  var LABEL_ROOM = 22;
  function barLabel(object, width, centred) {
    if (!object.label) return [];
    const words = fitWidth(object.label, width - 8, 14, 500);
    return [
      text(words, {
        x: centred ? width / 2 : 4,
        y: 15,
        "text-anchor": centred ? "middle" : "start",
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
        "data-part": "label"
      })
    ];
  }
  function drawBar(object, drawFrame) {
    const [width, height] = object.size;
    const horizontal = object.variant === "horizontal";
    const live = liveValueOf(object, "value", drawFrame);
    const reading = rangeReading(live, object);
    const ageRow = object.label && live?.staleMs !== void 0 ? LABEL_ROOM : 0;
    const top = (object.label ? LABEL_ROOM : 0) + ageRow;
    let track;
    if (horizontal) {
      const thickness = Math.min(18, Math.max(8, height - 24 - top));
      track = { x: 4, y: top + (height - top - thickness) / 2, width: width - 8 - CAPTION_BESIDE, height: thickness };
    } else {
      const thickness = Math.min(26, Math.max(8, width - 28));
      track = { x: (width - thickness) / 2, y: 6 + top, width: thickness, height: height - 6 - top - CAPTION_BELOW };
    }
    const children = barParts("", track, horizontal, live, reading, horizontal ? ACCENT_ACROSS : ACCENT_UP, object);
    children.push(...barLabel(object, width, !horizontal));
    children.push(
      ...horizontal ? inlineCaption(object, live, { x: track.x + track.width + 14, y: track.y + track.height / 2 + 6 }) : valueCaption(object, live, { x: width / 2, y: height - 22, "text-anchor": "middle" })
    );
    children.push(...stateMarks(object, live, width, height, 16 + ageRow));
    return el("g", { class: "hmi-object hmi-bar", "data-range": reading?.range, ...stateAttributes(live) }, children);
  }
  function valueCaption(object, live, at) {
    if (!live || live.state === "unknown") {
      return [text("?", { ...at, fill: token("unknown"), "font-size": 16, "font-weight": 600 })];
    }
    const unit = object.unit ?? object.bind.value?.unit;
    const number2 = text(formatValue(live.value, object), {
      ...at,
      fill: token("text"),
      "font-size": 16,
      "font-weight": 600,
      "font-variant-numeric": "tabular-nums"
    });
    return unit ? [number2, text(unit, { ...at, y: at.y + 17, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : [number2];
  }
  function inlineCaption(object, live, at) {
    if (!live || live.state === "unknown") {
      return [text("?", { ...at, fill: token("unknown"), "font-size": 16, "font-weight": 600 })];
    }
    const unit = object.unit ?? object.bind.value?.unit;
    return [
      run({ ...at, fill: token("text"), "font-size": 16, "font-weight": 600, "font-variant-numeric": "tabular-nums" }, [
        tspan(formatValue(live.value, object)),
        ...unit ? [tspan(unit, { dx: 4, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : []
      ])
    ];
  }
  function barParts(prefix, track, horizontal, live, reading, paint2, object) {
    const name = (part) => prefix ? `${prefix}-${part}` : part;
    const radius = Math.min(track.width, track.height) / 2;
    const stale = live?.staleMs !== void 0;
    const parts = [
      el("rect", {
        "data-part": name("track"),
        ...track,
        rx: radius,
        fill: token("raised"),
        stroke: stale ? token("stale") : void 0,
        "stroke-dasharray": stale ? "4 3" : void 0
      })
    ];
    if (!reading && live?.state === "unknown") parts.push(hatchBox(track, radius));
    if (reading) {
      const fill2 = horizontal ? { x: track.x, y: track.y, width: track.width * reading.fraction, height: track.height } : {
        x: track.x,
        y: track.y + track.height - track.height * reading.fraction,
        width: track.width,
        height: track.height * reading.fraction
      };
      parts.push(
        el("rect", {
          "data-part": name("fill"),
          "data-fraction": reading.fraction,
          ...fill2,
          rx: radius,
          fill: paint2,
          opacity: stale ? 0.45 : void 0
        })
      );
      if (reading.range) parts.push(rangeMark(name("range-mark"), track, horizontal, reading.range));
    }
    const range2 = object && alarmRange(object, live);
    if (range2) {
      const band = horizontal ? {
        x: track.x + track.width * range2.from,
        y: track.y + track.height + 5,
        width: track.width * (range2.to - range2.from),
        height: 3
      } : {
        x: track.x + track.width + 5,
        y: track.y + track.height * (1 - range2.to),
        width: 3,
        height: track.height * (range2.to - range2.from)
      };
      parts.push(
        el("rect", {
          ...band,
          "data-part": "normal-range",
          "data-from": range2.from,
          "data-to": range2.to,
          fill: token("muted"),
          opacity: 0.3,
          rx: 1.5
        })
      );
      for (const tick of range2.ticks) {
        const x = track.x + track.width * tick.fraction;
        const y = track.y + track.height * (1 - tick.fraction);
        parts.push(
          limitTick(
            tick,
            horizontal ? { x1: x, x2: x, y1: track.y - 3, y2: track.y + track.height + 3 } : { x1: track.x - 3, x2: track.x + track.width + 3, y1: y, y2: y },
            object
          )
        );
      }
    }
    return parts;
  }
  function rangeMark(part, track, horizontal, range2) {
    const size = 7;
    let points;
    if (horizontal) {
      const x = range2 === "over" ? track.x + track.width + 3 : track.x - 3;
      const direction = range2 === "over" ? 1 : -1;
      points = `${x},${track.y} ${x + direction * size},${track.y + track.height / 2} ${x},${track.y + track.height}`;
    } else {
      const y = range2 === "over" ? track.y - 3 : track.y + track.height + 3;
      const direction = range2 === "over" ? -1 : 1;
      const centre = track.x + track.width / 2;
      points = `${centre - size},${y} ${centre},${y + direction * size} ${centre + size},${y}`;
    }
    return el("polygon", { "data-part": part, points, fill: token("warning") });
  }

  // src/hmiBuilder/drawing/components/controls.ts
  var INSET = 16;
  var COMPACT_HEIGHT = 80;
  var MIN_LABEL = 32;
  var NUMBER_ROOM = 56;
  var UNAVAILABLE = 0.45;
  function unavailableContent(node) {
    if (String(node.attrs["data-part"] ?? "").startsWith("outcome")) return node;
    const attrs = { ...node.attrs };
    if (node.tag === "text" || node.tag === "tspan") {
      if (attrs.fill === token("page")) attrs.fill = token("text");
    } else if (["rect", "circle", "ellipse", "path", "line", "polyline", "polygon"].includes(node.tag)) {
      attrs.opacity = String(Number(attrs.opacity ?? 1) * UNAVAILABLE);
    }
    return { ...node, attrs, children: node.children.map(unavailableContent) };
  }
  var OUTCOMES = {
    pending: ["Sending\u2026", "pending"],
    applied: ["Applied", "muted"],
    confirmed: ["Confirmed", "accent"],
    refused: ["Refused", "alarm"],
    failed: ["Failed", "alarm"],
    "timed-out": ["Not confirmed", "warning"],
    unknown: ["Unknown", "unknown"]
  };
  var SIGNS = {
    pending: "\u2026",
    applied: "\u2713",
    confirmed: "\u2713",
    refused: "!",
    failed: "!",
    "timed-out": "!",
    unknown: "?"
  };
  function tooltip(words) {
    return { ...el("title"), text: words };
  }
  function heldReason(object, frame) {
    const permit = liveValueOf(object, "interlock", frame);
    if (!permit || permit.state === "unknown" || permit.value !== false) return void 0;
    const reason = liveValueOf(object, "interlock-reason", frame);
    return typeof reason?.value === "string" ? reason.value.trim() : "";
  }
  function controlGroup(kind, object, frame, attributes, children, variant) {
    const command = frame.commands?.[object.id];
    const [width, height] = object.size;
    const target = el("rect", { "data-part": "target", x: 0, y: 0, width, height, fill: "transparent" });
    const locked = frame.locked?.[object.id];
    const held = heldReason(object, frame);
    const content = frame.readOnly || locked || held !== void 0 ? children.map(unavailableContent) : children;
    if (locked) content.push(lockBadge(locked, width));
    if (held && !command) content.push(tooltip(held));
    return el(
      "g",
      {
        class: `hmi-object hmi-control hmi-${kind}${variant ? ` hmi-${kind}-${variant}` : ""}`,
        ...attributes,
        "data-outcome": command?.outcome,
        "data-readonly": frame.readOnly ? "true" : void 0,
        "data-locked": locked,
        "data-held": held !== void 0 ? "true" : void 0
      },
      [target, ...content]
    );
  }
  function roleWord(role) {
    return role === "engineer" ? say("Engineer") : role === "operator" ? say("Operator") : say("Locked");
  }
  function lockBadge(role, width) {
    const label = roleWord(role);
    const badge = Math.round(textWidth(label, 14, 600) + 36);
    const x = Math.max(0, width - badge - 8);
    return el("g", { "data-part": "lock", transform: `translate(${x} -12)` }, [
      el("rect", { x: 0, y: 0, width: badge, height: 24, rx: 12, fill: token("raised"), stroke: token("border") }),
      el("rect", { x: 10, y: 10, width: 10, height: 8, rx: 2, fill: token("muted") }),
      el("path", { d: "M12 10 V8 a3 3 0 0 1 6 0 V10", fill: "none", stroke: token("muted"), "stroke-width": 1.6 }),
      text(label, { x: 27, y: 17, fill: token("text"), "font-size": 14, "font-weight": 600 })
    ]);
  }
  function drawSetpoint(object, frame) {
    const [width, height] = object.size;
    if (height < COMPACT_HEIGHT) return drawCompactSetpoint(object, frame);
    const live = liveValueOf(object, "value", frame);
    const command = frame.commands?.[object.id];
    const unit = object.unit ?? object.bind.value?.unit;
    const children = [card(width, height, live)];
    if (object.label)
      children.push(
        text(fitWidth(object.label, width - 2 * INSET, 14, 500), {
          x: INSET,
          y: 26,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    const size = fontSize(height - NUMBER_ROOM, 1.03, 40);
    const number2 = {
      x: INSET,
      y: height - 24,
      "font-size": size,
      "font-weight": 600,
      "letter-spacing": -0.5,
      "font-variant-numeric": "tabular-nums"
    };
    if (live?.state === "unknown") {
      children.push(hatch(width, height));
      children.push(text("?", { ...number2, fill: token("unknown") }));
    } else if (live) {
      children.push(
        run({ ...number2, fill: token("text") }, [
          tspan(formatValue(live.value, object)),
          ...unit ? [tspan(unit, { dx: 6, fill: token("muted"), "font-size": 15, "font-weight": 500, "letter-spacing": 0 })] : []
        ])
      );
    }
    const footer = { x: INSET, y: height - 8, "font-size": 14 };
    if ((command?.outcome === "refused" || command?.outcome === "failed") && command.reason) {
      children.push(
        text(fitWidth(command.reason, width - 2 * INSET, 14), { ...footer, fill: token("alarm"), "data-part": "reason" })
      );
    } else if (object.min !== void 0 || object.max !== void 0) {
      const limits = `${object.min ?? "\u2026"} \u2013 ${object.max ?? "\u2026"}${unit ? ` ${unit}` : ""}`;
      children.push(
        text(fitWidth(limits, width - 2 * INSET - 36, 14), { ...footer, fill: token("muted"), "data-part": "limits" })
      );
    }
    children.push(editMark(width, height));
    if (command) {
      const [said, colour] = OUTCOMES[command.outcome];
      const word = say(said);
      children.push(chip(word, colour, width - 12, 12, false).node);
    }
    return controlGroup("setpoint", object, frame, stateAttributes(live), children);
  }
  function drawCompactSetpoint(object, frame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", frame);
    const command = frame.commands?.[object.id];
    const unit = object.unit ?? object.bind.value?.unit;
    const mark = Math.min(28, height - 12);
    const markX = width - 10 - mark;
    const baseline = Math.round(height / 2 + 5);
    const size = Math.min(22, fontSize(height, 0.42));
    const right = markX - 10;
    const shown = live?.state === "unknown" ? "?" : live ? formatValue(live.value, object) : "";
    const numberWidth = textWidth(shown, size, 600) + (live && live.state !== "unknown" && unit ? 4 + textWidth(unit, 14, 500) : 0);
    const room = right - numberWidth - 8 - INSET;
    const children = [card(width, height, live)];
    const number2 = {
      x: right,
      y: baseline,
      "font-size": size,
      "font-weight": 600,
      "text-anchor": "end",
      "font-variant-numeric": "tabular-nums"
    };
    if (live?.state === "unknown") {
      children.push(hatch(width, height));
      children.push(text("?", { ...number2, fill: token("unknown") }));
    } else if (live) {
      children.push(
        run({ ...number2, fill: token("text") }, [
          tspan(shown),
          ...unit ? [tspan(unit, { dx: 4, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : []
        ])
      );
    }
    const left = { x: INSET, y: baseline, "font-size": 14 };
    const fits = (words) => textWidth(words, 14, 600) <= room;
    const [said, colour] = command ? OUTCOMES[command.outcome] : ["Unknown", "muted"];
    const word = command ? say(said) : "";
    if ((command?.outcome === "refused" || command?.outcome === "failed") && command.reason && fits(command.reason)) {
      children.push(text(command.reason, { ...left, fill: token("alarm"), "font-weight": 600, "data-part": "reason" }));
    } else if (command && fits(word)) {
      children.push(text(word, { ...left, fill: token(colour), "font-weight": 600, "data-part": "outcome-word" }));
    } else if (!command && object.label && room >= MIN_LABEL) {
      children.push(text(fitWidth(object.label, room, 14, 500), { ...left, fill: token("muted"), "font-weight": 500 }));
    }
    const at = { x: markX, y: (height - mark) / 2, size: mark };
    children.push(command && !fits(word) ? outcomeSign(command, at) : editMark(width, height, at));
    children.push(...outcomeOutline(object, frame, { x: 0, y: 0, width, height }, 14));
    return controlGroup("setpoint", object, frame, stateAttributes(live), children);
  }
  function outcomeSign(command, at) {
    const colour = OUTCOMES[command.outcome][1];
    return el("g", { "data-part": "outcome-sign", transform: `translate(${at.x} ${at.y})` }, [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: at.size - 1,
        height: at.size - 1,
        rx: 8,
        fill: token("raised"),
        stroke: token(colour)
      }),
      text(SIGNS[command.outcome], {
        x: at.size / 2,
        y: at.size / 2 + 5,
        "text-anchor": "middle",
        fill: token(colour),
        "font-size": 15,
        "font-weight": 800
      })
    ]);
  }
  function editMark(width, height, at) {
    const size = at?.size ?? 28;
    const x = at?.x ?? width - INSET - size;
    const y = at?.y ?? Math.max(34, (height - size) / 2 + 4);
    const scale = size / 28;
    return el("g", { "data-part": "edit", transform: `translate(${x} ${y})` }, [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: size - 1,
        height: size - 1,
        rx: 8,
        fill: token("raised"),
        stroke: token("border")
      }),
      el("path", {
        d: "M9 19 L9 16 L17 8 L20 11 L12 19 Z M15.5 9.5 L18.5 12.5",
        fill: "none",
        stroke: token("accent"),
        "stroke-width": 1.6,
        "stroke-linejoin": "round",
        ...scale !== 1 ? { transform: `scale(${scale})` } : {}
      })
    ]);
  }
  function drawButton(object, frame) {
    const [width, height] = object.size;
    const command = frame.commands?.[object.id];
    const radius = Math.min(height / 2, 22);
    const stop = object.variant === "stop";
    const face = stop ? { fill: token("raised"), stroke: token("alarm"), "stroke-width": 1.5 } : { fill: ACCENT_ACROSS };
    const children = [
      el("rect", {
        "data-part": "face",
        x: 0.5,
        y: 0.5,
        width: Math.max(0, width - 1),
        height: Math.max(0, height - 1),
        rx: radius,
        ...face
      })
    ];
    const label = object.text ?? object.label ?? "";
    const size = Math.min(16, fontSize(height, 0.4));
    const cx = width - Math.min(radius, 18);
    const [from, to] = command ? [12, cx - 9 - 4] : [12, width - 12];
    children.push(
      text(fitWidth(label, to - from, size, 650), {
        x: (from + to) / 2,
        y: height / 2 + 5,
        "text-anchor": "middle",
        fill: token(stop ? "alarm" : "page"),
        "font-size": size,
        "font-weight": 650
      })
    );
    if (command) {
      const [said, colour] = OUTCOMES[command.outcome];
      const word = say(said);
      children.push(
        el("rect", {
          "data-part": "outcome",
          x: -1.5,
          y: -1.5,
          width: width + 3,
          height: height + 3,
          rx: radius + 2,
          fill: "none",
          stroke: token(colour),
          "stroke-width": 2
        })
      );
      children.push(
        el("g", { "data-part": "outcome-sign" }, [
          el("circle", { cx, cy: height / 2, r: 9, fill: token("page"), "fill-opacity": 0.85 }),
          text(SIGNS[command.outcome], {
            x: cx,
            y: height / 2 + 4.5,
            "text-anchor": "middle",
            fill: token(colour),
            "font-size": 13,
            "font-weight": 800
          })
        ])
      );
      const refusal = (command.outcome === "refused" || command.outcome === "failed") && command.reason;
      children.push(tooltip(refusal || word));
    }
    return controlGroup(object.type, object, frame, {}, children, stop ? "stop" : void 0);
  }
  function outcomeOutline(object, frame, box, radius) {
    const command = frame.commands?.[object.id];
    if (!command) return [];
    const [word, colour] = OUTCOMES[command.outcome];
    return [
      el("rect", {
        "data-part": "outcome",
        x: box.x - 1.5,
        y: box.y - 1.5,
        width: box.width + 3,
        height: box.height + 3,
        rx: radius + 2,
        fill: "none",
        stroke: token(colour),
        "stroke-width": 2
      }),
      tooltip(word)
    ];
  }
  function fitted(content, width, perChar) {
    const room = Math.max(4, Math.floor(width / perChar));
    return content.length <= room ? content : `${content.slice(0, room - 1)}\u2026`;
  }

  // src/hmiBuilder/drawing/components/choices.ts
  var MODES = ["Manual", "Auto", "Track"];
  function staleOutline(live, box, radius) {
    if (live?.staleMs === void 0) return [];
    return [
      el("rect", {
        "data-part": "stale",
        x: box.x + 0.5,
        y: box.y + 0.5,
        width: box.width - 1,
        height: box.height - 1,
        rx: radius,
        fill: "none",
        stroke: token("stale"),
        "stroke-dasharray": "5 4"
      })
    ];
  }
  var SWITCH_TRACK_HEIGHT = 32;
  function drawSwitch(object, frame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", frame);
    const trackHeight = Math.min(height, SWITCH_TRACK_HEIGHT);
    const top = (height - trackHeight) / 2;
    const [offWord, onWord] = object.options?.length === 2 ? object.options.map((option) => translate(option)) : [say("Off"), say("On")];
    const wordWidth = Math.max(textWidth(offWord, 14, 650), textWidth(onWord, 14, 650));
    const trackWidth = Math.min(
      width,
      Math.max(64, Math.round(trackHeight * 2.2), Math.round(wordWidth + trackHeight + 8))
    );
    const x = width - trackWidth;
    const radius = trackHeight / 2;
    const unknown = !live || live.state === "unknown";
    const on2 = !unknown && isOn(live.value);
    const children = [];
    if (object.label && x >= 40) {
      children.push(
        text(fitted(object.label, x - 12, 7.4), {
          x: 0,
          y: height / 2 + 5,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    children.push(
      el("rect", {
        "data-part": "track",
        x: x + 0.5,
        y: top + 0.5,
        width: trackWidth - 1,
        height: trackHeight - 1,
        rx: radius,
        fill: on2 ? ACCENT_ACROSS : token("raised"),
        stroke: on2 ? "none" : token("border")
      })
    );
    const words = { y: height / 2 + 5, "font-size": 14, "font-weight": 650 };
    if (unknown) {
      children.push(hatchBox({ x: x + 1.5, y: top + 1.5, width: trackWidth - 3, height: trackHeight - 3 }, radius));
      children.push(text("?", { ...words, x: x + trackWidth / 2, "text-anchor": "middle", fill: token("unknown") }));
    } else {
      const knob = radius - 3;
      const cx = on2 ? width - 3 - knob : x + 3 + knob;
      children.push(
        el("circle", { "data-part": "knob", cx, cy: height / 2, r: knob, fill: on2 ? token("page") : token("muted") })
      );
      children.push(
        on2 ? text(onWord, {
          ...words,
          x: x + (trackWidth - 2 * knob - 6) / 2,
          "text-anchor": "middle",
          fill: token("page")
        }) : text(offWord, {
          ...words,
          x: x + 6 + 2 * knob + (trackWidth - 2 * knob - 6) / 2,
          "text-anchor": "middle",
          fill: token("muted")
        })
      );
    }
    const track = { x, y: top, width: trackWidth, height: trackHeight };
    children.push(...staleOutline(live, track, radius), ...outcomeOutline(object, frame, track, radius));
    return controlGroup(
      "switch",
      object,
      frame,
      { ...stateAttributes(live), "data-on": unknown ? void 0 : String(on2) },
      children
    );
  }
  function optionWords(option) {
    const words = option.replace(/([a-z0-9])([A-Z])/g, "$1 $2").split(" ");
    return translate(
      words.map(
        (word, index) => index === 0 || word.length > 1 && word === word.toUpperCase() ? word : word.toLowerCase()
      ).join(" ")
    );
  }
  function optionOf(value, options) {
    if (typeof value === "string") return options.find((option) => option.toLowerCase() === value.toLowerCase()) ?? value;
    if (typeof value === "number" && Number.isInteger(value) && value >= 0 && value < options.length)
      return options[value];
    return value === null ? "?" : String(value);
  }
  function drawSelector(object, frame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", frame);
    const unknown = !live || live.state === "unknown";
    const radius = Math.min(12, height / 2);
    const children = [
      el("rect", {
        "data-part": "box",
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: radius,
        fill: token("raised"),
        stroke: token("border")
      })
    ];
    const baseline = height / 2 + 5;
    let left = 14;
    if (object.label) {
      const label = fitWidth(object.label, width / 2 - 20, 14, 500);
      children.push(text(label, { x: left, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 500 }));
      left += Math.round(textWidth(label, 14, 500)) + 14;
    }
    if (unknown) {
      children.push(hatchBox({ x: 1.5, y: 1.5, width: width - 3, height: height - 3 }, radius));
      children.push(text("?", { x: left, y: baseline, fill: token("unknown"), "font-size": 16, "font-weight": 650 }));
    } else {
      const choice = fitWidth(optionWords(optionOf(live.value, object.options ?? [])), width - left - 40, 16, 650);
      children.push(
        text(choice, {
          x: left,
          y: baseline,
          fill: token("accent"),
          "font-size": 16,
          "font-weight": 650,
          "data-part": "choice"
        })
      );
    }
    const cx = width - 20;
    children.push(
      el("path", {
        "data-part": "chevron",
        d: `M${cx - 5} ${height / 2 - 2} L${cx} ${height / 2 + 3} L${cx + 5} ${height / 2 - 2}`,
        fill: "none",
        stroke: token("muted"),
        "stroke-width": 1.8,
        "stroke-linecap": "round",
        "stroke-linejoin": "round"
      })
    );
    const whole = { x: 0, y: 0, width, height };
    children.push(...staleOutline(live, whole, radius), ...outcomeOutline(object, frame, whole, radius));
    return controlGroup("selector", object, frame, stateAttributes(live), children);
  }
  function drawModeSelector(object, frame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "mode", frame);
    const unknown = !live || live.state === "unknown";
    const modes = object.options && object.options.length > 0 ? object.options : MODES;
    const active = unknown || typeof live.value !== "string" ? void 0 : live.value.toLowerCase();
    const radius = height / 2;
    const segment = (width - 8) / modes.length;
    const children = [
      el("rect", {
        "data-part": "track",
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: radius,
        fill: token("raised"),
        stroke: token("border")
      })
    ];
    modes.forEach((mode, index) => {
      const lit = mode.toLowerCase() === active;
      const x = 4 + index * segment;
      children.push(
        el("g", { "data-option": mode, "data-active": lit ? "true" : void 0 }, [
          el("rect", {
            x,
            y: 4,
            width: segment,
            height: height - 8,
            rx: radius - 4,
            fill: lit ? ACCENT_ACROSS : "transparent"
          }),
          text(fitted(optionWords(mode), segment - 12, 7.6), {
            x: x + segment / 2,
            y: height / 2 + 5,
            "text-anchor": "middle",
            "font-size": 14,
            "font-weight": lit ? 700 : 500,
            fill: lit ? token("page") : token("muted")
          })
        ])
      );
    });
    if (unknown) children.push(hatchBox({ x: 1.5, y: 1.5, width: width - 3, height: height - 3 }, radius));
    const whole = { x: 0, y: 0, width, height };
    children.push(...staleOutline(live, whole, radius), ...outcomeOutline(object, frame, whole, radius));
    return controlGroup("mode-selector", object, frame, stateAttributes(live), children);
  }

  // src/hmiBuilder/drawing/symbols/rules.ts
  function applyRules(rules, value, frame, object) {
    const parts = {};
    const set = (part, view) => {
      parts[part] = { ...parts[part], ...view };
    };
    for (const rule of rules) {
      if ("spin" in rule) {
        const spin = turning(value(rule.spin), frame);
        if (spin) set(rule.part, { spin });
      } else if ("stroke" in rule) {
        const stroke = turning(value(rule.stroke), frame);
        if (stroke) set(rule.part, { stroke });
      } else if ("angle" in rule) {
        const angle = number(value(rule.angle));
        if (angle !== void 0) set(rule.part, { angle });
      } else if ("level" in rule) {
        const level = share(value(rule.level), rule.range ?? range(object));
        if (level !== void 0) set(rule.part, { level });
      } else if ("travel" in rule) {
        const travel = share(value(rule.travel), rule.range ?? range(object));
        if (travel !== void 0) {
          const moved2 = rule.move ? [travel * rule.move[0], travel * rule.move[1]] : void 0;
          set(rule.part, moved2 ? { travel, moved: moved2 } : { travel });
        }
      } else if ("blink" in rule) {
        const alarm = value(rule.blink);
        if (on(alarm) && observed(alarm)) set(rule.part, { blink: true });
      } else if ("show" in rule) {
        const text2 = shownBoundNumber(value, rule.show, frame, object);
        if (text2 !== void 0) set(rule.part, { text: text2 });
      } else if ("alarm" in rule) {
        if (on(value(rule.alarm))) set(rule.part, { alarm: true });
      } else if ("equals" in rule) {
        const shown = number(value(rule.equals));
        set(rule.part, { hidden: shown === void 0 || shown !== rule.value });
      } else if ("active" in rule) {
        const bound = value(rule.active);
        const working = bound !== void 0 && bound.state !== "unknown" && (typeof bound.value === "boolean" ? bound.value : Number(bound.value) > 0);
        if (working) set(rule.part, { active: true });
      }
    }
    return parts;
  }
  function number(value) {
    const parsed = Number(value?.value);
    return value === void 0 || value.state === "unknown" || value.value === null || !Number.isFinite(parsed) ? void 0 : parsed;
  }
  function range(object) {
    return [object.min ?? 0, object.max ?? 100];
  }
  function share(value, [min, max]) {
    const parsed = number(value);
    if (parsed === void 0 || max === min) return void 0;
    return Math.min(1, Math.max(0, (parsed - min) / (max - min)));
  }

  // src/hmiBuilder/drawing/components/equipmentState.ts
  function readEquipment(definition, object, frame) {
    const value = (name) => liveValueOf(object, name, frame);
    const values = definition.deciding.map(value).filter((candidate) => candidate !== void 0);
    const base2 = {
      manual: on(value("manual")),
      outOfService: on(value("out-of-service")),
      live: stalest(values) ?? values.find((candidate) => candidate.state === "unknown") ?? values[0]
    };
    const own = { ...object.variant ? { variant: object.variant } : {}, ...object.tag ? { tag: object.tag } : {} };
    if (values.some((candidate) => candidate.state === "unknown")) {
      return { ...base2, word: "?", fault: false, view: { unknown: true, ...own } };
    }
    const reading = definition.read(value, frame, on(value("fault")), object);
    const parts = applyRules(definition.rules ?? [], value, frame, object);
    const view = { ...reading.view, ...Object.keys(parts).length > 0 ? { parts } : {}, ...own };
    return { ...base2, ...reading, view };
  }
  function stalest(values) {
    return values.filter((value) => value.staleMs !== void 0).sort((a, b) => (b.staleMs ?? 0) - (a.staleMs ?? 0))[0];
  }

  // src/hmiBuilder/drawing/components/equipment.ts
  var OUT_OF_SERVICE_OPACITY = 0.45;
  var TAG_BASELINE = 20;
  var STATE_BASELINE = 40;
  var GAP = 8;
  function drawEquipment(object, frame) {
    const type = object.type.replace(":", "-");
    const definition = symbolFor(object.type);
    if (!definition) return el("g", { class: "hmi-object", "data-object": object.id });
    const [width, height] = object.size;
    const [symbolWidth, symbolHeight] = definition.size;
    const beside = definition.label === "beside";
    const { left, top, scale, area } = symbolPlacement(object.size, definition.size, definition.label);
    const reading = readEquipment(definition, object, frame);
    const children = [
      el(
        "g",
        {
          "data-part": "symbol",
          transform: `translate(${left} ${top}) scale(${round(scale)})`,
          opacity: reading.outOfService ? OUT_OF_SERVICE_OPACITY : void 0
        },
        definition.draw(reading.view)
      )
    ];
    if (reading.live?.staleMs !== void 0) {
      const box = beside ? {
        x: 1,
        y: Math.max(1, top - 3),
        width: round(symbolWidth * scale + 6),
        height: Math.min(height - 2, round(symbolHeight * scale + 6))
      } : {
        x: Math.max(1, left - 4),
        y: Math.max(1, top - 4),
        width: Math.min(width - 2, symbolWidth * scale + 8),
        height: Math.min(area, symbolHeight * scale + 6)
      };
      children.push(
        el("rect", {
          "data-part": "stale-outline",
          ...box,
          rx: 10,
          fill: "none",
          stroke: token("stale"),
          "stroke-width": 1.5,
          "stroke-dasharray": "6 4"
        })
      );
    }
    const tag = object.tag ?? object.label;
    if (beside) {
      const start = round(symbolWidth * scale + GAP + 6);
      const first = round((height - LABEL_HEIGHT) / 2);
      if (tag) {
        children.push(
          text(fitWidth(tag, width - start, 14, 600), {
            "data-part": "tag",
            x: start,
            y: first + TAG_BASELINE,
            fill: token("text"),
            "font-size": 14,
            "font-weight": 600,
            "text-anchor": "start"
          })
        );
      }
      children.push(...stateLine(reading, width, first + STATE_BASELINE, start));
    } else {
      if (tag) {
        children.push(
          text(fitWidth(tag, width, 14, 600), {
            "data-part": "tag",
            x: width / 2,
            y: area + TAG_BASELINE,
            fill: token("text"),
            "font-size": 14,
            "font-weight": 600,
            "text-anchor": "middle"
          })
        );
      }
      children.push(...stateLine(reading, width, area + STATE_BASELINE));
    }
    return el(
      "g",
      {
        class: `hmi-object hmi-equipment hmi-${type}`,
        "data-equipment": reading.state,
        "data-manual": reading.manual ? "true" : void 0,
        "data-out-of-service": reading.outOfService ? "true" : void 0,
        ...stateAttributes(reading.live)
      },
      children
    );
  }
  function stateLine(reading, width, baseline, start) {
    const signs = [];
    if (reading.fault) signs.push({ name: "fault", label: "!", paint: "alarm", ink: "page" });
    if (reading.manual) signs.push({ name: "manual", label: say("M"), paint: "raised", ink: "text", edge: "muted" });
    if (reading.outOfService)
      signs.push({ name: "out-of-service", label: say("OOS"), paint: "raised", ink: "text", edge: "muted" });
    if (reading.live?.staleMs !== void 0)
      signs.push({ name: "stale", label: formatAge(reading.live.staleMs), paint: "raised", ink: "stale", edge: "stale" });
    const widths = signs.map((sign) => Math.max(22, Math.round(textWidth(sign.label, 14, 700) + 14)));
    const signsWidth = signs.length === 0 ? 0 : widths.reduce((sum, signWidth) => sum + signWidth, 0) + GAP * (signs.length - 1);
    const room = width - (start ?? 0);
    const shown = reading.word ? fitWidth(reading.word, Math.max(0, room - (signs.length === 0 ? 0 : signsWidth + GAP)), 14, 500) : "";
    const word = (x2, anchor) => text(shown, {
      "data-part": "state",
      x: round(x2),
      y: baseline,
      fill: reading.fault ? token("alarm") : token("muted"),
      "font-size": 14,
      "font-weight": 500,
      "text-anchor": anchor
    });
    if (signs.length === 0) return shown ? [start === void 0 ? word(width / 2, "middle") : word(start, "start")] : [];
    const wordWidth = shown ? textWidth(shown, 14, 500) : 0;
    const total = wordWidth + (shown ? GAP : 0) + signsWidth;
    const wordEnd = start === void 0 ? Math.max(wordWidth, (width - total) / 2 + wordWidth) : start + wordWidth;
    const nodes = shown ? [word(wordEnd, "end")] : [];
    let x = shown ? wordEnd + GAP : start ?? Math.max(0, (width - signsWidth) / 2);
    signs.forEach((sign, index) => {
      const signWidth = widths[index];
      const top = baseline - 16;
      nodes.push(
        el("g", { "data-sign": sign.name }, [
          el("rect", {
            x: round(x),
            y: top,
            width: signWidth,
            height: 22,
            rx: 11,
            fill: token(sign.paint),
            stroke: sign.edge ? token(sign.edge) : void 0,
            "stroke-width": sign.edge ? 1 : void 0
          }),
          text(sign.label, {
            x: round(x + signWidth / 2),
            y: top + 16,
            fill: token(sign.ink),
            "font-size": 14,
            "font-weight": 700,
            "text-anchor": "middle"
          })
        ])
      );
      x += signWidth + GAP;
    });
    return nodes;
  }

  // src/hmiBuilder/drawing/components/eventLog.ts
  var KINDS = {
    raised: "Raised",
    cleared: "Cleared",
    acknowledged: "Acknowledged",
    shelved: "Shelved",
    unshelved: "Unshelved",
    suppressed: "Suppressed",
    unsuppressed: "Unsuppressed",
    "out-of-service": "Out of service",
    "in-service": "In service",
    reset: "Reset",
    comment: "Comment"
  };
  var OUTCOMES2 = {
    confirmed: { label: "Confirmed", colour: "accent", sign: "\u2713" },
    applied: { label: "Applied", colour: "accent", sign: "\u2713" },
    refused: { label: "Refused", colour: "alarm", sign: "!" },
    "timed-out": { label: "Timed out", colour: "warning", sign: "!" }
  };
  var PAD2 = 16;
  var HEADER2 = 56;
  var ROW2 = 52;
  var FOOTER2 = 48;
  function drawEventLog(object, frame) {
    const [width, height] = object.size;
    const alarms = frame.alarms;
    const stale = alarms?.stale === true;
    const records = [...alarms?.records ?? []].sort((a, b) => b.seq - a.seq);
    const children = [
      el("rect", {
        "data-part": "card",
        x: 0.5,
        y: 0.5,
        width: Math.max(0, width - 1),
        height: Math.max(0, height - 1),
        rx: 14,
        fill: token("surface"),
        stroke: stale ? token("stale") : token("border"),
        "stroke-dasharray": stale ? "6 4" : void 0
      }),
      text(say("Events"), { x: PAD2 + 4, y: 33, fill: token("text"), "font-size": 15, "font-weight": 650 })
    ];
    const headerFits = PAD2 + 80 + PAGER_WIDTH <= width - PAD2;
    const room = (footer) => Math.max(1, Math.floor((height - HEADER2 - (footer ? FOOTER2 : 8)) / ROW2));
    const paged = records.length > room(false);
    const capacity = paged && !headerFits ? room(true) : room(false);
    const lastPage = Math.max(0, Math.floor((records.length - 1) / capacity) * capacity);
    const first = Math.min(Math.max(0, alarms?.views?.[object.id]?.first ?? 0), lastPage);
    const shown = records.slice(first, first + capacity);
    if (paged)
      children.push(
        pager("event", first, shown.length, records.length, width - PAD2, headerFits ? 10 : height - FOOTER2 + 4)
      );
    if (shown.length === 0) {
      children.push(
        text(say("No events yet"), {
          x: width / 2,
          y: HEADER2 + (height - HEADER2) / 2,
          fill: token("muted"),
          "font-size": 15,
          "font-weight": 550,
          "text-anchor": "middle"
        })
      );
    }
    shown.forEach((record, index) => children.push(drawRecord(record, HEADER2 + index * ROW2, width)));
    return el("g", { class: "hmi-object hmi-event-log", "data-stale": stale ? "true" : void 0 }, children);
  }
  function drawRecord(record, top, width) {
    const outcome = record.kind === "command" ? OUTCOMES2[record.outcome ?? ""] : void 0;
    const kind = KINDS[record.kind];
    const what = record.kind === "command" ? outcome ? say(outcome.label) : record.outcome ?? say("Command") : kind ? say(kind) : record.kind;
    const detail = record.detail ? record.kind === "comment" ? `\u201C${record.detail}\u201D` : record.detail : void 0;
    const below = [timeOfDay(record.time_ms), record.path, record.user, detail].filter((part) => Boolean(part)).join(" \xB7 ");
    const right = width - PAD2;
    return el("g", { "data-part": "event-row", "data-seq": record.seq, "data-kind": record.kind }, [
      el("line", { x1: PAD2, y1: top, x2: width - PAD2, y2: top, stroke: token("line"), "stroke-width": 1 }),
      markOf(record, outcome, PAD2 + 12, top + ROW2 / 2),
      text(fitWidth(`${what} \xB7 ${translate(record.message || record.path)}`, right - 48, 15, 600), {
        x: 48,
        y: top + 22,
        fill: token("text"),
        "font-size": 15,
        "font-weight": 600
      }),
      text(fitWidth(below, right - 48, 14), {
        x: 48,
        y: top + 42,
        fill: token("muted"),
        "font-size": 14,
        "font-variant-numeric": "tabular-nums"
      })
    ]);
  }
  function markOf(record, outcome, x, y) {
    if (record.kind !== "command") return priorityMark(alarmPriority(record.priority), x, y, 20);
    const colour = outcome?.colour ?? "muted";
    return el("g", { "data-part": "outcome-mark", "data-outcome": record.outcome ?? "" }, [
      el("circle", { cx: x, cy: y, r: 9, fill: "none", stroke: token(colour), "stroke-width": 2 }),
      text(outcome?.sign ?? "\xB7", {
        x,
        y: y + 4.5,
        fill: token(colour),
        "font-size": 12,
        "font-weight": 800,
        "text-anchor": "middle"
      })
    ]);
  }

  // src/hmiBuilder/drawing/components/gauge.ts
  var START = 135;
  var SWEEP = 270;
  function point(cx, cy, radius, degrees) {
    const radians = degrees * Math.PI / 180;
    return [Number((cx + radius * Math.cos(radians)).toFixed(2)), Number((cy + radius * Math.sin(radians)).toFixed(2))];
  }
  function arc2(cx, cy, radius, from, sweep) {
    const [x0, y0] = point(cx, cy, radius, from);
    const [x1, y1] = point(cx, cy, radius, from + sweep);
    return `M ${x0} ${y0} A ${radius} ${radius} 0 ${sweep > 180 ? 1 : 0} 1 ${x1} ${y1}`;
  }
  function drawGauge(object, drawFrame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", drawFrame);
    const reading = rangeReading(live, object);
    const cx = width / 2;
    const cy = height / 2;
    const stroke = Math.max(8, Math.round(Math.min(width, height) * 0.075));
    const radius = Math.min(width, height) / 2 - stroke - 4;
    const stale = live?.staleMs !== void 0;
    const children = [
      el("path", {
        "data-part": "track",
        d: arc2(cx, cy, radius, START, SWEEP),
        fill: "none",
        stroke: token("line"),
        "stroke-width": stroke,
        "stroke-linecap": "round"
      })
    ];
    if (reading) {
      children.push(
        el("path", {
          "data-part": "fill",
          "data-fraction": reading.fraction,
          d: arc2(cx, cy, radius, START, SWEEP * reading.fraction),
          fill: "none",
          stroke: reading.range ? token("warning") : ACCENT_ACROSS,
          "stroke-width": stroke,
          "stroke-linecap": "round",
          filter: stale ? void 0 : GLOW,
          // A stale value fades; its age is shown in the corner.
          opacity: stale ? 0.45 : void 0
        })
      );
    }
    const limits = alarmRange(object, live);
    if (limits) {
      children.push(
        el("path", {
          "data-part": "normal-range",
          "data-from": limits.from,
          "data-to": limits.to,
          d: arc2(
            cx,
            cy,
            Math.max(1, radius - stroke - 5),
            START + SWEEP * limits.from,
            SWEEP * (limits.to - limits.from)
          ),
          fill: "none",
          stroke: token("muted"),
          opacity: 0.3,
          "stroke-width": 3,
          "stroke-linecap": "round"
        })
      );
      for (const tick of limits.ticks) {
        const angle = START + SWEEP * tick.fraction;
        const [x1, y1] = point(cx, cy, radius - stroke / 2 - 3, angle);
        const [x2, y2] = point(cx, cy, radius + stroke / 2 + 3, angle);
        children.push(limitTick(tick, { x1, y1, x2, y2 }, object));
      }
    }
    const target = live?.state === "commanded" && typeof live.target === "number" ? rangeReading({ value: live.target, state: "confirmed" }, object) : void 0;
    if (target) {
      const angle = START + SWEEP * target.fraction;
      const [x0, y0] = point(cx, cy, radius - stroke, angle);
      const [x1, y1] = point(cx, cy, radius + stroke, angle);
      children.push(
        el("line", {
          "data-part": "target",
          "data-fraction": target.fraction,
          x1: x0,
          y1: y0,
          x2: x1,
          y2: y1,
          stroke: token("pending"),
          "stroke-width": 3,
          "stroke-linecap": "round"
        })
      );
    }
    const size = Math.max(14, Math.round(radius * 0.42));
    const unknown = !live || live.state === "unknown";
    children.push(
      text(unknown ? "?" : formatValue(live.value, object), {
        x: cx,
        y: cy + size * 0.35,
        fill: unknown ? token("unknown") : token("text"),
        "font-size": size,
        "font-weight": 600,
        "letter-spacing": -0.5,
        "text-anchor": "middle",
        "font-variant-numeric": "tabular-nums"
      })
    );
    const unit = object.unit ?? object.bind.value?.unit;
    let below = cy + size * 0.35 + 18;
    if (unit && !unknown) {
      children.push(
        text(unit, {
          x: cx,
          y: below,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500,
          "text-anchor": "middle"
        })
      );
      below += 18;
    }
    if (live?.state === "commanded")
      children.push(commandedMark(object, live, { x: cx, y: below, "text-anchor": "middle" }));
    if (live?.state === "estimated") children.push(estimatedMark({ x: cx, y: below, "text-anchor": "middle" }));
    if (live && stale) children.push(staleMark(live, { x: width - 6, y: 16, "text-anchor": "end" }));
    const [minX, minY] = point(cx, cy, radius, START);
    const [maxX, maxY] = point(cx, cy, radius, START + SWEEP);
    children.push(
      text(formatValue(object.min ?? 0, {}), {
        x: minX,
        y: minY + stroke + 12,
        fill: token("muted"),
        "font-size": 14,
        "text-anchor": "middle"
      })
    );
    children.push(
      text(formatValue(object.max ?? 100, {}), {
        x: maxX,
        y: maxY + stroke + 12,
        fill: token("muted"),
        "font-size": 14,
        "text-anchor": "middle"
      })
    );
    return el("g", { class: "hmi-object hmi-gauge", "data-range": reading?.range, ...stateAttributes(live) }, children);
  }

  // src/hmiBuilder/drawing/components/image.ts
  function drawImage(object, drawFrame) {
    const [width, height] = object.size;
    if (!object.image) {
      return el("g", { class: "hmi-object hmi-image" }, [
        el("rect", {
          x: 0.5,
          y: 0.5,
          width: width - 1,
          height: height - 1,
          rx: 14,
          fill: "none",
          stroke: token("border"),
          "stroke-dasharray": "6 5"
        }),
        text(say("No image"), {
          x: width / 2,
          y: height / 2 + 5,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500,
          "text-anchor": "middle"
        })
      ]);
    }
    const href = drawFrame.asset ? drawFrame.asset(object.image) : object.image;
    return el("g", { class: "hmi-object hmi-image" }, [
      el("image", { href, x: 0, y: 0, width, height, preserveAspectRatio: "xMidYMid meet" })
    ]);
  }

  // src/hmiBuilder/drawing/components/junction.ts
  function drawJunction(object) {
    const [width, height] = object.size;
    return el("g", { class: "hmi-object hmi-junction" }, [
      el("circle", {
        "data-part": "dot",
        cx: width / 2,
        cy: height / 2,
        r: Math.max(3, Math.min(width, height) / 4),
        fill: token("muted")
      })
    ]);
  }

  // src/hmiBuilder/drawing/components/layout.ts
  function drawPanel(object) {
    const [width, height] = object.size;
    const children = [card(width, height, void 0, 16)];
    if (object.label) {
      children.push(
        text(fitWidth(object.label, width - 36, 15, 600), {
          x: 18,
          y: 30,
          fill: token("text"),
          "font-size": 15,
          "font-weight": 600
        })
      );
    }
    return el("g", { class: "hmi-object hmi-panel" }, children);
  }
  function drawGroup(object) {
    const [width, height] = object.size;
    const children = [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: 14,
        fill: "none",
        stroke: token("border"),
        "stroke-width": 1.5
      })
    ];
    if (object.label) {
      const room = width - 32 - 0.3 * Array.from(object.label).length;
      children.push(
        text(fitWidth(object.label, room, 14, 600), {
          x: 16,
          y: 26,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 600,
          "letter-spacing": 0.3
        })
      );
    }
    return el("g", { class: "hmi-object hmi-group" }, children);
  }
  function drawLine(object) {
    const [width, height] = object.size;
    return el("g", { class: "hmi-object hmi-line" }, [
      el("line", {
        x1: 0,
        y1: height / 2,
        x2: width,
        y2: height / 2,
        stroke: token("border"),
        "stroke-width": Math.max(1, Math.min(height, 4)),
        "stroke-linecap": "round"
      })
    ]);
  }
  function drawPageButton(object) {
    const [width, height] = object.size;
    const label = object.text ?? object.label ?? object.click?.page ?? "";
    return el("g", { class: "hmi-object hmi-page-button" }, [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: (height - 1) / 2,
        fill: token("raised"),
        stroke: token("border")
      }),
      text(fitWidth(label, width - 24, 15, 600), {
        x: width / 2,
        y: height / 2 + 5,
        fill: token("text"),
        "font-size": 15,
        "font-weight": 600,
        "text-anchor": "middle"
      })
    ]);
  }

  // src/hmiBuilder/drawing/components/placeholder.ts
  function drawPlaceholder(object) {
    const [width, height] = object.size;
    return el("g", { class: "hmi-object hmi-placeholder", "data-placeholder": "true" }, [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: Math.max(0, width - 1),
        height: Math.max(0, height - 1),
        rx: 14,
        fill: "none",
        stroke: token("border"),
        "stroke-width": 1.5,
        "stroke-dasharray": "6 5"
      }),
      text(object.type, {
        x: width / 2,
        y: height / 2 + 5,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
        "text-anchor": "middle"
      })
    ]);
  }

  // src/hmiBuilder/drawing/components/slider.ts
  var THUMB = 11;
  var NUMBER_ROOM2 = 64;
  var LABEL_ROOM2 = 22;
  function sliderLimits(object) {
    return [object.min ?? 0, object.max ?? 100];
  }
  function sliderTrack(object) {
    const [width, height] = object.size;
    const label = object.label ? LABEL_ROOM2 : 0;
    if (object.variant === "vertical") return [width / 2, height - THUMB - 4, width / 2, label + NUMBER_ROOM2 / 2 + THUMB];
    const y = object.label ? Math.max(height / 2, label + (height - label) / 2) : height / 2;
    return [THUMB + 4, y, width - NUMBER_ROOM2 - THUMB, y];
  }
  function drawSlider(object, frame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", frame);
    const draft = frame.drafts?.[object.id];
    const [min, max] = sliderLimits(object);
    const vertical = object.variant === "vertical";
    const shown = draft ?? (typeof live?.value === "number" ? live.value : void 0);
    const unknown = draft === void 0 && (!live || live.state === "unknown" || shown === void 0);
    const share2 = shown === void 0 || max === min ? 0 : Math.min(1, Math.max(0, (shown - min) / (max - min)));
    const [x1, y1, x2, y2] = sliderTrack(object);
    const at = { x: x1 + (x2 - x1) * share2, y: y1 + (y2 - y1) * share2 };
    const children = [
      el("line", {
        "data-part": "track",
        x1,
        y1,
        x2,
        y2,
        stroke: token("raised"),
        "stroke-width": 6,
        "stroke-linecap": "round"
      })
    ];
    const number2 = vertical ? { x: width / 2, y: (object.label ? LABEL_ROOM2 : 0) + 18, "text-anchor": "middle" } : { x: width - 4, y: y1 + 6, "text-anchor": "end" };
    if (object.label) {
      const room = vertical ? width - 8 : width - (x1 - THUMB) - 4;
      children.push(
        text(fitWidth(object.label, room, 14, 500), {
          x: vertical ? width / 2 : x1 - THUMB,
          y: 16,
          "text-anchor": vertical ? "middle" : void 0,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    if (unknown) {
      const box = vertical ? { x: width / 2 - 6, y: y2 - 3, width: 12, height: y1 - y2 + 6 } : { x: x1 - 3, y: y1 - 6, width: x2 - x1 + 6, height: 12 };
      children.push(hatchBox(box, 6));
      children.push(text("?", { ...number2, fill: token("unknown"), "font-size": 16, "font-weight": 650 }));
    } else {
      children.push(
        el("line", {
          "data-part": "fill",
          x1,
          y1,
          x2: at.x,
          y2: at.y,
          stroke: token("accent"),
          "stroke-width": 6,
          "stroke-linecap": "round"
        })
      );
      children.push(
        el("circle", {
          "data-part": "thumb",
          cx: at.x,
          cy: at.y,
          r: THUMB,
          fill: token("text"),
          stroke: token("accent"),
          "stroke-width": draft === void 0 ? 2 : 3
        })
      );
      const unit = object.unit ?? object.bind.value?.unit;
      const words = `${formatValue(shown ?? null, object)}${unit ? ` ${unit}` : ""}`;
      children.push(
        text(words, {
          ...number2,
          fill: draft === void 0 ? token("text") : token("accent"),
          "font-size": 16,
          "font-weight": 650,
          "font-variant-numeric": "tabular-nums"
        })
      );
    }
    if (live?.staleMs !== void 0 && draft === void 0) {
      children.push(
        el("rect", {
          "data-part": "stale",
          x: 0.5,
          y: 0.5,
          width: width - 1,
          height: height - 1,
          rx: 10,
          fill: "none",
          stroke: token("stale"),
          "stroke-dasharray": "5 4"
        })
      );
    }
    children.push(...outcomeOutline(object, frame, { x: 0, y: 0, width, height }, 10));
    return controlGroup(
      "slider",
      object,
      frame,
      { ...stateAttributes(live), "data-draft": draft === void 0 ? void 0 : "true" },
      children
    );
  }

  // src/hmiBuilder/drawing/components/splitBar.ts
  var CAPTION_BELOW2 = 30;
  function drawSplitBar(object, drawFrame) {
    const [width, height] = object.size;
    const thickness = Math.min(24, Math.max(8, (width - 36) / 2));
    const gap = width - 2 * thickness - 24;
    const top = object.label ? LABEL_ROOM : 0;
    const hasUnit = ["first", "second"].some((property) => Boolean(withValueFormat(object, drawFrame, property).unit));
    const trackHeight = height - 12 - top - (hasUnit ? 48 : CAPTION_BELOW2);
    const children = [...barLabel(object, width, true)];
    const outputs = [
      ["first", 12, SECOND_UP],
      ["second", 12 + thickness + gap, ACCENT_UP]
    ];
    for (const [output, x, paint2] of outputs) {
      const track = { x, y: 6 + top, width: thickness, height: trackHeight };
      const live = liveValueOf(object, output, drawFrame);
      const formatted = withValueFormat(object, drawFrame, output);
      children.push(...barParts(output, track, false, live, rangeReading(live, formatted), paint2, formatted));
      const shown = live && live.state !== "unknown" ? formatValue(live.value, formatted) : "?";
      children.push(
        text(shown, {
          x: x + thickness / 2,
          y: height - (hasUnit ? 27 : 10),
          fill: shown === "?" ? token("unknown") : token("text"),
          "font-size": 16,
          "font-weight": 600,
          "text-anchor": "middle",
          "font-variant-numeric": "tabular-nums"
        })
      );
      if (formatted.unit && shown !== "?")
        children.push(
          text(formatted.unit, {
            x: x + thickness / 2,
            y: height - 10,
            fill: token("muted"),
            "font-size": 14,
            "text-anchor": "middle"
          })
        );
    }
    const request = rangeReading(liveValueOf(object, "request", drawFrame), object);
    if (request) {
      const y = 6 + top + trackHeight - trackHeight * request.fraction;
      children.push(
        el("line", {
          "data-part": "request",
          "data-fraction": request.fraction,
          x1: 4,
          y1: y,
          x2: width - 4,
          y2: y,
          stroke: token("text"),
          "stroke-width": 3,
          "stroke-linecap": "round"
        })
      );
    }
    return el("g", { class: "hmi-object hmi-split-bar" }, children);
  }

  // src/hmiBuilder/drawing/components/status.ts
  function drawStatus(object, drawFrame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "state", drawFrame);
    const unknown = live?.state === "unknown";
    const on2 = live !== void 0 && !unknown && isOn(live.value);
    const alarm = object.variant === "alarm";
    const radius = Math.max(6, Math.round(height * 0.2));
    const centre = { x: height / 2, y: height / 2 };
    const lit = on2 ? alarm ? token("alarm") : token("accent") : void 0;
    const children = [
      el("rect", {
        "data-part": "pill",
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: (height - 1) / 2,
        fill: token("surface"),
        stroke: live?.staleMs !== void 0 ? token("stale") : token("border")
      }),
      el("circle", {
        "data-part": "lamp",
        cx: centre.x,
        cy: centre.y,
        r: radius,
        fill: unknown ? "url(#hmi-unknown-hatch)" : lit ?? "none",
        stroke: unknown ? token("unknown") : lit ? void 0 : token("muted"),
        "stroke-width": lit ? void 0 : 2,
        filter: lit ? GLOW : void 0
      })
    ];
    if (on2 && alarm) {
      children.push(
        text("!", {
          x: centre.x,
          y: centre.y + 5,
          fill: token("page"),
          "font-size": 14,
          "font-weight": 800,
          "text-anchor": "middle",
          "data-part": "alarm-mark"
        })
      );
    }
    const chips = stateChipRow(object, live, width, (height - 22) / 2);
    const caption = object.label ?? object.tag;
    if (caption && width > height + 12) {
      const end2 = chips.nodes.length > 0 ? chips.left - 8 : width - Math.max(12, height / 3);
      children.push(
        text(fitWidth(caption, end2 - (height - 2), 14, 500), {
          x: height - 2,
          y: height / 2 + 5,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    if (unknown) {
      children.push(
        text("?", {
          x: centre.x,
          y: centre.y + 5,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 700,
          "text-anchor": "middle"
        })
      );
    }
    children.push(...chips.nodes);
    return el(
      "g",
      {
        class: "hmi-object hmi-status",
        "data-on": live === void 0 || unknown ? void 0 : String(on2),
        ...stateAttributes(live)
      },
      children
    );
  }

  // src/hmiBuilder/drawing/components/table.ts
  var PAD3 = 16;
  var ROW3 = 30;
  var CHAR = 7.6;
  function isStructure(value) {
    return value !== null && value !== void 0 && typeof value === "object" && !Array.isArray(value);
  }
  function cell(value, object) {
    if (value === void 0) return { kind: "value", content: "" };
    if (typeof value === "boolean") return { kind: "flag", on: value };
    if (Array.isArray(value)) return { kind: "value", content: `[${value.length}]` };
    if (isStructure(value)) return { kind: "value", content: `{${Object.keys(value).length}}` };
    const content = formatValue(value, object);
    return { kind: typeof value === "string" && content === value ? "text" : "value", content };
  }
  function holdsNothing(value) {
    if (value === "" || value === false || value === 0) return true;
    if (Array.isArray(value)) return value.every(holdsNothing);
    return isStructure(value) && Object.values(value).every(holdsNothing);
  }
  function gridOf(value, lowerBounds, object) {
    if (isStructure(value)) {
      return {
        rows: Object.entries(value).map(([field, item]) => ({ label: optionWords(field), cells: [cell(item, object)] })),
        empty: 0
      };
    }
    if (!Array.isArray(value)) return { rows: [{ label: "", cells: [cell(value, object)] }], empty: 0 };
    const index = (position, dimension) => String((lowerBounds?.[dimension] ?? 0) + position);
    if (value.length > 0 && value.every(isStructure)) {
      const fields = [...new Set(value.flatMap((item) => Object.keys(item)))];
      let used = value.length;
      while (used > 0 && holdsNothing(value[used - 1])) used -= 1;
      return {
        header: { label: "#", cells: fields.map(optionWords) },
        rows: value.slice(0, used).map((item, position) => ({
          label: index(position, 0),
          cells: fields.map((field) => cell(item[field], object))
        })),
        empty: value.length - used
      };
    }
    if (value.length > 0 && value.every((item) => Array.isArray(item))) {
      const columns = value.reduce((most, row) => Math.max(most, row.length), 0);
      const positions = Array.from({ length: columns }, (_, column) => column);
      return {
        header: { cells: positions.map((column) => index(column, 1)) },
        rows: value.map((row, position) => ({
          label: index(position, 0),
          cells: positions.map((column) => cell(row[column], object))
        })),
        empty: 0
      };
    }
    return {
      rows: value.map((item, position) => ({ label: index(position, 0), cells: [cell(item, object)] })),
      empty: 0
    };
  }
  function clip(content, width) {
    return fitText(content, width, CHAR);
  }
  function flagMark(on2, x, y) {
    return el("circle", {
      "data-part": "flag",
      "data-on": String(on2),
      cx: x,
      cy: y,
      r: 6,
      fill: on2 ? token("accent") : "none",
      stroke: on2 ? token("accent") : token("muted"),
      "stroke-width": 1.5
    });
  }
  function kindOf(rows, column) {
    const cells = rows.map((row) => row.cells[column]).filter((item) => item !== void 0);
    if (cells.some((item) => item.kind === "text")) return "text";
    return cells.length > 0 && cells.every((item) => item.kind === "flag") ? "flag" : "value";
  }
  function drawTable(object, frame) {
    const [width, height] = object.size;
    let live = liveValueOf(object, "value", frame);
    if (object.bind["row-count"] && live && Array.isArray(live.value)) {
      const count = liveValueOf(object, "row-count", frame);
      const number3 = typeof count?.value === "number" ? count.value : typeof count?.value === "string" && /^\d+$/.test(count.value) ? Number(count.value) : NaN;
      live = count?.state === "confirmed" && Number.isInteger(number3) && number3 >= 0 && number3 <= live.value.length ? { ...live, value: live.value.slice(0, number3) } : { ...live, state: "unknown", value: null };
    }
    const children = [card(width, height, live)];
    const row = stateChipRow(object, live, width);
    const chips = row.nodes;
    let top = PAD3;
    if (object.label) {
      const room2 = Math.min(width - 2 * PAD3, row.left - 8 - PAD3);
      children.push(
        text(fitWidth(object.label, room2, 15, 600), {
          x: PAD3,
          y: top + 14,
          fill: token("text"),
          "font-size": 15,
          "font-weight": 600
        })
      );
      top += 28;
    } else if (chips.length > 0) {
      top = 42;
    }
    children.push(...chips);
    const group = (nodes) => el("g", { class: "hmi-object hmi-table", ...stateAttributes(live) }, nodes);
    const centred = { x: width / 2, y: (top + height) / 2 + 8, "text-anchor": "middle" };
    if (!live || live.state === "unknown" || live.value === null) {
      if (live) children.push(hatch(width, height));
      children.push(text("?", { ...centred, fill: token("unknown"), "font-size": 28, "font-weight": 600 }));
      return group(children);
    }
    if (Array.isArray(live.value) && live.value.length === 0) {
      children.push(text(say("Empty"), { ...centred, fill: token("muted"), "font-size": 15, "font-weight": 500 }));
      return group(children);
    }
    const grid = gridOf(live.value, live.lowerBounds, object);
    const columns = grid.rows.reduce((most, row2) => Math.max(most, row2.cells.length), grid.header?.cells.length ?? 0);
    const longestLabel = grid.rows.reduce(
      (most, row2) => Math.max(most, row2.label.length),
      grid.header?.label?.length ?? 0
    );
    const labelWidth = Math.min(width * 0.45, Math.max(24, longestLabel * CHAR + 16));
    const cellsLeft = PAD3 + labelWidth;
    const room = width - PAD3 - cellsLeft;
    const kinds = Array.from({ length: columns }, (_, column) => grid.header ? kindOf(grid.rows, column) : "value");
    const weights = Array.from(
      { length: columns },
      (_, column) => grid.header ? grid.rows.reduce(
        (most, row2) => {
          const item = row2.cells[column];
          return Math.max(most, item === void 0 ? 0 : item.kind === "flag" ? 2 : item.content.length);
        },
        Math.max(2, (grid.header.cells[column] ?? "").length)
      ) : 1
    );
    const total = weights.reduce((sum, weight) => sum + weight, 0) || 1;
    const widths = weights.map((weight) => room * weight / total);
    const lefts = widths.map((_, column) => cellsLeft + widths.slice(0, column).reduce((sum, part) => sum + part, 0));
    const number2 = { "font-size": 15, "font-weight": 600, "text-anchor": "end", "font-variant-numeric": "tabular-nums" };
    const separator = (y) => el("line", { x1: PAD3, y1: y, x2: width - PAD3, y2: y, stroke: token("line"), "stroke-width": 1 });
    const anchor = (column) => {
      const left = lefts[column] ?? cellsLeft;
      const span = widths[column] ?? room;
      if (kinds[column] === "text") return { x: left + 8 };
      if (kinds[column] === "flag") return { x: left + span / 2, "text-anchor": "middle" };
      return { x: left + span, "text-anchor": "end" };
    };
    if (grid.header) {
      const baseline = top + 19;
      const titles = [];
      if (grid.header.label) {
        titles.push(
          text(grid.header.label, { x: PAD3, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 600 })
        );
      }
      grid.header.cells.forEach((title, column) => {
        titles.push(
          text(clip(title, widths[column] - 8), {
            ...anchor(column),
            y: baseline,
            fill: token("muted"),
            "font-size": 14,
            "font-weight": 600
          })
        );
      });
      children.push(
        el("g", { "data-part": "header" }, [
          ...titles,
          el("line", {
            x1: PAD3,
            y1: top + ROW3,
            x2: width - PAD3,
            y2: top + ROW3,
            stroke: token("border"),
            "stroke-width": 1
          })
        ])
      );
      top += ROW3;
    }
    const fit = Math.max(0, Math.floor((height - PAD3 - top) / ROW3));
    const notes = grid.empty > 0 ? 1 : 0;
    const space = Math.max(0, fit - notes);
    const overflow = grid.rows.length > space;
    const shown = overflow ? Math.max(0, space - 1) : grid.rows.length;
    grid.rows.slice(0, shown).forEach((row2, position) => {
      const rowTop = top + position * ROW3;
      const baseline = rowTop + 20;
      const nodes = [];
      if (row2.label !== "") {
        nodes.push(
          text(clip(row2.label, labelWidth - 8), {
            x: PAD3,
            y: baseline,
            fill: token("muted"),
            "font-size": 14,
            "font-weight": 500
          })
        );
      }
      row2.cells.forEach((item, column) => {
        const at = anchor(column);
        if (item.kind === "flag") {
          nodes.push(
            flagMark(
              item.on,
              grid.header ? Number(at.x) : (lefts[column] ?? cellsLeft) + (widths[column] ?? room) - 8,
              rowTop + ROW3 / 2
            )
          );
        } else if (item.content !== "") {
          const style = item.kind === "text" ? { "font-size": 15, "font-weight": 500 } : number2;
          nodes.push(
            text(clip(item.content, (widths[column] ?? room) - 8), { ...style, ...at, y: baseline, fill: token("text") })
          );
        }
      });
      if (position < shown - 1 || overflow || notes > 0) nodes.push(separator(rowTop + ROW3));
      children.push(el("g", { "data-part": "row", "data-y": rowTop }, nodes));
    });
    const note = (part, words, line) => {
      const rowTop = top + line * ROW3;
      children.push(
        el("g", { "data-part": part, "data-y": rowTop }, [
          text(words, { x: PAD3, y: rowTop + 20, fill: token("muted"), "font-size": 14, "font-weight": 500 })
        ])
      );
    };
    if (overflow) note("more", say("+{count} more", { count: grid.rows.length - shown }), shown);
    if (grid.empty > 0) note("empty", say("Empty rows: {count}", { count: grid.empty }), shown + (overflow ? 1 : 0));
    return group(children);
  }

  // src/hmiBuilder/drawing/components/text.ts
  var VARIANTS = {
    heading: { size: 26, weight: 650, colour: "text" },
    body: { size: 18, weight: 500, colour: "text" },
    caption: { size: 14, weight: 500, colour: "muted" }
  };
  function drawText(object, drawFrame) {
    const height = object.size[1];
    const variant = VARIANTS[object.variant ?? "body"] ?? VARIANTS.body;
    const live = liveValueOf(object, "text", drawFrame);
    const unknown = live?.state === "unknown";
    const content = live ? unknown ? "?" : formatValue(live.value, object) : object.text ?? "";
    return el("g", { class: "hmi-object hmi-text", ...stateAttributes(live) }, [
      // Text that does not fit is cut with "…" (§14.5 item 6).
      text(fitWidth(content, object.size[0], variant.size, variant.weight), {
        x: 0,
        y: height / 2 + variant.size * 0.35,
        fill: unknown ? token("unknown") : token(variant.colour),
        "font-size": variant.size,
        "font-weight": variant.weight,
        "letter-spacing": variant === VARIANTS.heading ? -0.3 : void 0
      })
    ]);
  }

  // src/hmiBuilder/drawing/components/trend.ts
  var LIMITS = ["high-limit", "low-limit"];
  var NOT_PENS = [...LIMITS, "visible"];
  var PEN_COLOURS = ["pen-1", "pen-2", "pen-3", "pen-4"];
  var DEFAULT_SPAN_SECONDS = 300;
  var PAD4 = 16;
  var AXIS2 = 52;
  var LANE = 22;
  var LANE_GAP = 6;
  var LANES_TOP = 12;
  var LEGEND_ROW = 24;
  var TOOLS = 30;
  var CHIP2 = 26;
  var RANGES = [300, 3600, 28800, 86400];
  var TIME_ROW = 22;
  var CHAR2 = 9;
  var round5 = (value) => Math.round(value * 10) / 10;
  var clamp2 = (value) => Math.min(1, Math.max(0, value));
  function pensOf(object, frame) {
    return Object.keys(object.bind).filter((name) => !NOT_PENS.includes(name)).sort().map((name, index) => {
      const binding = object.bind[name];
      const formatted = withValueFormat(object, frame, name);
      const live = liveValueOf(object, name, frame) ?? { value: null, state: "unknown" };
      const samples = (frame.history?.[binding.path] ?? []).filter(([time, value]) => Number.isFinite(time) && Number.isFinite(value)).sort((first, second) => first[0] - second[0]);
      return {
        name,
        colour: PEN_COLOURS[index % PEN_COLOURS.length],
        dashed: index >= PEN_COLOURS.length,
        isBoolean: typeof live.value === "boolean",
        unit: binding.unit ?? formatted.unit,
        decimals: formatted.decimals,
        live,
        samples
      };
    });
  }
  function visible(samples, window, held) {
    const inside2 = samples.filter(([time]) => time >= window.start && time <= window.end);
    const before = samples.filter(([time]) => time < window.start).pop();
    if (!before || inside2.length === 0 || inside2[0][0] === window.start) return inside2;
    const [beforeTime, beforeValue] = before;
    const [firstTime, firstValue] = inside2[0];
    const atStart = held ? beforeValue : beforeValue + (firstValue - beforeValue) * (window.start - beforeTime) / (firstTime - beforeTime);
    return [[window.start, atStart], ...inside2];
  }
  function niceStep(rough) {
    const power = 10 ** Math.floor(Math.log10(rough));
    const fraction = rough / power;
    const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 2.5 ? 2.5 : fraction <= 5 ? 5 : 10;
    return nice * power;
  }
  function axisOf(object, values) {
    const { min, max } = object;
    if (min !== void 0 && max !== void 0 && Number.isFinite(min) && Number.isFinite(max) && max > min) {
      return { low: min, high: max, ticks: [min, (min + max) / 2, max] };
    }
    let low = values.length > 0 ? values.reduce((least, value) => Math.min(least, value)) : 0;
    let high = values.length > 0 ? values.reduce((most, value) => Math.max(most, value)) : 100;
    if (high === low) {
      low -= 1;
      high += 1;
    }
    const margin = (high - low) * 0.1;
    const step = niceStep((high - low + 2 * margin) / 3);
    low = Math.floor((low - margin) / step) * step;
    high = Math.ceil((high + margin) / step) * step;
    const ticks = [];
    for (let tick = low; tick <= high + step / 2; tick += step) ticks.push(Number(tick.toFixed(10)));
    return { low, high, ticks };
  }
  function spanLabel(seconds) {
    if (seconds % 3600 === 0) return `\u2212${say("{hours} h", { hours: seconds / 3600 })}`;
    if (seconds % 60 === 0) return `\u2212${say("{minutes} min", { minutes: seconds / 60 })}`;
    return `\u2212${say("{seconds} s", { seconds })}`;
  }
  function atCursor(pen, cursorMs) {
    const sample2 = pen.samples.filter(([time]) => time <= cursorMs).pop();
    if (!sample2) return { value: null, state: "unknown" };
    return { value: pen.isBoolean ? sample2[1] !== 0 : sample2[1], state: "confirmed" };
  }
  function legendSpans(object, pen, cursorMs) {
    const spans = [tspan(pen.name, { dx: 6, fill: token("muted"), "font-weight": 500 })];
    const live = cursorMs === void 0 ? pen.live : atCursor(pen, cursorMs);
    if (live.state === "unknown") {
      spans.push(tspan("?", { dx: 6, fill: token("unknown"), "font-weight": 600 }));
      return spans;
    }
    spans.push(tspan(formatValue(live.value, pen), { dx: 6, fill: token("text"), "font-weight": 600 }));
    if (pen.unit) spans.push(tspan(pen.unit, { dx: 3, fill: token("muted") }));
    if (live.staleMs !== void 0)
      spans.push(tspan(formatAge(live.staleMs), { dx: 6, fill: token("stale"), "font-weight": 600 }));
    if (live.state === "estimated") spans.push(tspan("est.", { dx: 6, fill: token("muted"), "font-style": "italic" }));
    return spans;
  }
  function legendOf(object, pens, width, top, view) {
    const rows = [];
    let used = Infinity;
    for (const pen of pens) {
      const spans = legendSpans(object, pen, view.cursorMs);
      const characters = spans.reduce((sum, span) => sum + (span.text ?? "").length, 0);
      const entryWidth = 14 + characters * CHAR2 + spans.length * 6;
      const gap = used === Infinity ? 0 : 18;
      if (used + gap + entryWidth > width - 2 * PAD4) {
        rows.push([]);
        used = 0;
      } else {
        used += gap;
      }
      const hidden = view.hidden?.includes(pen.name) === true;
      const dim = hidden ? { "fill-opacity": 0.4 } : {};
      const marker2 = tspan(pen.dashed ? "\u254C" : "\u25CF", { dx: used === 0 ? void 0 : 18, fill: token(pen.colour), ...dim });
      rows[rows.length - 1].push(
        marker2,
        el(
          "tspan",
          {
            "data-part": "legend",
            "data-pen": pen.name,
            "data-hidden": hidden ? "true" : void 0,
            cursor: "pointer",
            ...dim
          },
          spans
        )
      );
      used += entryWidth;
    }
    const nodes = rows.map(
      (entries, row) => run({ x: PAD4, y: top + row * LEGEND_ROW + 15, "font-size": 14, "font-variant-numeric": "tabular-nums" }, entries)
    );
    return { nodes, height: rows.length * LEGEND_ROW };
  }
  function rangeWords(seconds) {
    return seconds % 3600 === 0 ? say("{hours} h", { hours: seconds / 3600 }) : say("{minutes} min", { minutes: seconds / 60 });
  }
  var ICONS = {
    export: "M15 7 v10 M10 13 l5 5 l5 -5 M8 21 h14",
    expand: "M8 12 v-4 h4 M22 12 v-4 h-4 M8 15 v4 h4 M22 15 v4 h-4"
  };
  function toolsOf(spanSeconds, paused, width, top, labelled) {
    const live = paused ? [{ part: "trend-live", attrs: { "data-trend-action": "live" }, label: say("Back to live"), selected: true }] : [];
    const ranges = RANGES.map((seconds) => ({
      part: "trend-range",
      attrs: { "data-trend-range": String(seconds) },
      label: rangeWords(seconds),
      selected: seconds === spanSeconds
    }));
    const tools = [
      {
        part: "trend-tool",
        attrs: { "data-trend-action": "export", "aria-label": say("Export") },
        label: "",
        selected: false,
        icon: ICONS.export
      },
      {
        part: "trend-tool",
        attrs: { "data-trend-action": "expand", "aria-label": say("Expand") },
        label: "",
        selected: false,
        icon: ICONS.expand
      }
    ];
    const room = width - 2 * PAD4 - (labelled ? 96 : 0);
    const widthOf = (chip2) => chip2.icon ? 30 : Math.round(textWidth(chip2.label, 14, 600) + 20);
    const totalOf = (list) => list.reduce((sum, chip2) => sum + widthOf(chip2) + 6, -6);
    const chips = [[...live, ...ranges, ...tools], [...live, ...tools], live].find((list) => totalOf(list) <= room) ?? [];
    if (chips.length === 0) return { nodes: [], left: width - PAD4 };
    const widths = chips.map(widthOf);
    const total = totalOf(chips);
    let x = width - PAD4 - total;
    const left = x;
    const nodes = chips.map((chip2, index) => {
      const face = chip2.icon ? el("path", {
        d: chip2.icon,
        transform: `translate(${x} ${top - 2})`,
        fill: "none",
        stroke: token("muted"),
        "stroke-width": 1.8,
        "stroke-linecap": "round",
        "stroke-linejoin": "round"
      }) : text(chip2.label, {
        x: x + widths[index] / 2,
        y: top + 18,
        fill: chip2.selected ? token("accent") : token("muted"),
        "font-size": 14,
        "font-weight": 600,
        "text-anchor": "middle"
      });
      const node = el(
        "g",
        { "data-part": chip2.part, ...chip2.attrs, "data-selected": chip2.selected ? "true" : void 0, cursor: "pointer" },
        [
          el("rect", {
            x,
            y: top,
            width: widths[index],
            height: CHIP2,
            rx: CHIP2 / 2,
            fill: chip2.selected ? token("raised") : "transparent",
            stroke: chip2.selected ? token("accent") : token("border")
          }),
          face
        ]
      );
      x += widths[index] + 6;
      return node;
    });
    return { nodes, left };
  }
  function drawTrend(object, frame) {
    const [width, height] = object.size;
    const pens = pensOf(object, frame);
    const view = frame.trendViews?.[object.id] ?? {};
    const newest = pens.reduce(
      (latest, pen) => Math.max(latest, pen.samples[pen.samples.length - 1]?.[0] ?? -Infinity),
      -Infinity
    );
    const paused = view.endMs !== void 0;
    const end2 = view.endMs ?? frame.nowMs ?? (Number.isFinite(newest) ? newest : 0);
    const spanSeconds = Math.max(
      1,
      view.spanMs !== void 0 ? view.spanMs / 1e3 : object.span ?? DEFAULT_SPAN_SECONDS
    );
    const window = { start: end2 - spanSeconds * 1e3, end: end2 };
    const lanes = pens.filter((pen) => pen.isBoolean);
    const children = [card(width, height, void 0)];
    let top = PAD4;
    const badgeWidth = equipmentBadgeWidth(object, frame);
    const toolWidth = width - (badgeWidth > 0 ? badgeWidth + 8 : 0);
    const tools = toolsOf(spanSeconds, paused, toolWidth, top, Boolean(object.label));
    children.push(...tools.nodes);
    if (object.label) {
      children.push(
        text(fitWidth(object.label, tools.left - 2 * PAD4, 15, 600), {
          x: PAD4,
          y: top + 17,
          fill: token("text"),
          "font-size": 15,
          "font-weight": 600
        })
      );
    }
    if (object.label || tools.nodes.length > 0) top += TOOLS;
    const legend = legendOf(object, pens, width, top, view);
    const hidden = new Set(view.hidden ?? []);
    children.push(...legend.nodes);
    top += legend.height + 8;
    const lanesHeight = lanes.length > 0 ? LANES_TOP + lanes.length * (LANE + LANE_GAP) - LANE_GAP : 0;
    const plot = {
      x: PAD4 + AXIS2,
      y: top,
      width: Math.max(1, width - 2 * PAD4 - AXIS2),
      height: Math.max(1, height - PAD4 - TIME_ROW - lanesHeight - top)
    };
    const shown = new Map(pens.map((pen) => [pen.name, visible(pen.samples, window, pen.isBoolean)]));
    const limits = LIMITS.map((name) => [name, liveValueOf(object, name, frame)]).filter(
      (entry) => entry[1] !== void 0 && entry[1].state !== "unknown" && typeof entry[1].value === "number" && Number.isFinite(entry[1].value)
    );
    const numeric = pens.filter((pen) => !pen.isBoolean && !hidden.has(pen.name));
    const sharing = (pen) => pen.unit === void 0 || object.bind[pen.name].range ? [pen] : numeric.filter((other) => other.unit === pen.unit && !object.bind[other.name].range);
    const axes = new Map(
      numeric.map((pen) => {
        const range2 = object.bind[pen.name].range;
        const bounds = range2 ? { min: range2[0], max: range2[1] } : object;
        const scaled2 = sharing(pen);
        return [
          pen.name,
          axisOf(bounds, [
            ...scaled2.flatMap((member) => (shown.get(member.name) ?? []).map(([, value]) => value)),
            ...scaled2.includes(numeric[0]) ? limits.map(([, live]) => live.value) : []
          ])
        ];
      })
    );
    const axis = axes.get(numeric[0]?.name) ?? axisOf(object, []);
    const xOf = (time) => round5(plot.x + clamp2((time - window.start) / (window.end - window.start)) * plot.width);
    const yOf = (value, scale = axis) => round5(plot.y + plot.height - clamp2((value - scale.low) / (scale.high - scale.low)) * plot.height);
    children.push(
      el("rect", { "data-part": "plot", x: plot.x, y: plot.y, width: plot.width, height: plot.height, fill: "none" })
    );
    const tickCount = Math.min(axis.ticks.length, Math.floor(plot.height / 20) + 1);
    const ticks = tickCount <= 1 ? [axis.ticks[0]] : Array.from(
      { length: tickCount },
      (_, index) => axis.ticks[Math.round(index * (axis.ticks.length - 1) / (tickCount - 1))]
    );
    for (const tick of ticks) {
      const y = yOf(tick);
      children.push(
        el("line", { x1: plot.x, y1: y, x2: plot.x + plot.width, y2: y, stroke: token("border"), "stroke-width": 1 })
      );
      children.push(
        text(formatValue(tick, {}), {
          x: plot.x - 10,
          y: y + 5,
          fill: token(numeric[0]?.colour ?? "muted"),
          "data-axis-pen": numeric[0]?.name,
          "font-size": 14,
          "text-anchor": "end",
          "font-variant-numeric": "tabular-nums"
        })
      );
    }
    for (const [name, live] of limits) {
      if (live.value < axis.low || live.value > axis.high) continue;
      const y = yOf(live.value);
      const band = name === "high-limit" ? { y: plot.y, height: round5(y - plot.y) } : { y, height: round5(plot.y + plot.height - y) };
      children.push(
        el("rect", {
          "data-part": "limit-band",
          x: plot.x,
          width: plot.width,
          ...band,
          fill: token("warning"),
          "fill-opacity": 0.06
        })
      );
      children.push(
        el("line", {
          "data-part": "limit",
          "data-limit": name,
          x1: plot.x,
          y1: y,
          x2: plot.x + plot.width,
          y2: y,
          stroke: token("warning"),
          "stroke-width": 1.5,
          "stroke-dasharray": "5 4"
        })
      );
    }
    const stroke = (pen) => ({
      "data-pen": pen.name,
      fill: "none",
      stroke: token(pen.colour),
      "stroke-width": 2,
      "stroke-linejoin": "round",
      "stroke-linecap": "round",
      "stroke-dasharray": pen.dashed ? "6 4" : void 0
    });
    for (const pen of numeric) {
      const samples = shown.get(pen.name) ?? [];
      if (samples.length === 0) continue;
      const points = samples.map(([time, value]) => `${xOf(time)},${yOf(value, axes.get(pen.name))}`).join(" ");
      children.push(el("polyline", { ...stroke(pen), points }));
    }
    lanes.forEach((pen, index) => {
      const laneTop = plot.y + plot.height + LANES_TOP + index * (LANE + LANE_GAP);
      const [high, low] = [laneTop + 5, laneTop + LANE - 5];
      children.push(
        el("rect", {
          "data-part": "lane",
          x: plot.x,
          y: laneTop,
          width: plot.width,
          height: LANE,
          rx: 6,
          fill: token("raised")
        })
      );
      children.push(
        text(pen.name, {
          x: plot.x - 10,
          y: laneTop + LANE / 2 + 5,
          fill: token("muted"),
          "font-size": 14,
          "text-anchor": "end"
        })
      );
      const points = [];
      let previous;
      for (const [time, value] of hidden.has(pen.name) ? [] : shown.get(pen.name) ?? []) {
        const x = xOf(time);
        const y = value ? high : low;
        if (previous !== void 0 && previous !== y) points.push([x, previous]);
        points.push([x, y]);
        previous = y;
      }
      if (points.length > 0) {
        children.push(el("polyline", { ...stroke(pen), points: points.map(([x, y]) => `${x},${y}`).join(" ") }));
      }
    });
    if (view.cursorMs !== void 0 && view.cursorMs >= window.start && view.cursorMs <= window.end) {
      const x = xOf(view.cursorMs);
      children.push(
        el("line", {
          "data-part": "trend-cursor",
          x1: x,
          y1: plot.y,
          x2: x,
          y2: plot.y + plot.height,
          stroke: token("text"),
          "stroke-opacity": 0.7,
          "stroke-width": 1,
          "stroke-dasharray": "3 3"
        })
      );
      const right = x > plot.x + plot.width - 80;
      children.push(
        text(timeOfDay(view.cursorMs), {
          x: right ? x - 6 : x + 6,
          y: plot.y + 16,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 600,
          "text-anchor": right ? "end" : void 0,
          "font-variant-numeric": "tabular-nums"
        })
      );
    }
    if (view.box) {
      const [from, to] = [Math.min(view.box.fromMs, view.box.toMs), Math.max(view.box.fromMs, view.box.toMs)];
      children.push(
        el("rect", {
          "data-part": "trend-box",
          x: xOf(from),
          y: plot.y,
          width: Math.max(1, round5(xOf(to) - xOf(from))),
          height: plot.height,
          fill: token("accent"),
          "fill-opacity": 0.14,
          stroke: token("accent")
        })
      );
    }
    children.push(
      el("rect", {
        "data-part": "trend-time",
        x: plot.x,
        y: plot.y + plot.height + lanesHeight,
        width: plot.width,
        height: TIME_ROW + 4,
        fill: "transparent",
        cursor: "ew-resize"
      })
    );
    const timeBaseline = plot.y + plot.height + lanesHeight + 18;
    const label = { y: timeBaseline, fill: token("muted"), "font-size": 14 };
    children.push(text(paused ? timeOfDay(window.start) : spanLabel(spanSeconds), { ...label, x: plot.x }));
    children.push(
      text(paused ? timeOfDay(window.end) : say("now"), { ...label, x: plot.x + plot.width, "text-anchor": "end" })
    );
    return el(
      "g",
      {
        class: "hmi-object hmi-trend",
        "data-window-start": window.start,
        "data-window-end": window.end,
        "data-paused": paused ? "true" : void 0
      },
      children
    );
  }

  // src/hmiBuilder/drawing/components/value.ts
  var INSET2 = 16;
  var COMPACT_HEIGHT2 = 72;
  function drawValue(object, drawFrame) {
    const [width, height] = object.size;
    const live = liveValueOf(object, "value", drawFrame);
    if (object.label && height < COMPACT_HEIGHT2) return drawCompactValue(object, live, width, height);
    const labelled = Boolean(object.label);
    const size = fontSize(labelled ? height - 26 : height, labelled ? 0.62 : 0.5);
    const baseline = height - Math.max(12, Math.round((height - (labelled ? 26 : 0) - size) / 2));
    const children = [card(width, height, live)];
    const chips = stateChipRow(object, live, width);
    if (object.label) {
      const room = Math.min(width - 2 * INSET2, chips.left - 8 - INSET2);
      children.push(
        text(fitWidth(object.label, room, 14, 500), {
          x: INSET2,
          y: 27,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    const big = {
      x: INSET2,
      y: baseline,
      "font-size": size,
      "font-weight": 600,
      "letter-spacing": -0.5,
      "font-variant-numeric": "tabular-nums"
    };
    if (live?.state === "unknown") {
      children.push(hatch(width, height));
      children.push(text("?", { ...big, fill: token("unknown") }));
    } else if (live) {
      const unit = object.unit ?? object.bind.value?.unit;
      children.push(
        run({ ...big, fill: token("text") }, [
          tspan(formatValue(live.value, object)),
          ...unit ? [tspan(unit, { dx: 6, fill: token("muted"), "font-size": 15, "font-weight": 500, "letter-spacing": 0 })] : []
        ])
      );
    } else if (object.text) {
      children.push(text(fitWidth(object.text, width - 2 * INSET2, size, 600), { ...big, fill: token("text") }));
    }
    children.push(...chips.nodes);
    return el("g", { class: "hmi-object hmi-value", ...stateAttributes(live) }, children);
  }
  function drawCompactValue(object, live, width, height) {
    const baseline = height / 2 + 6;
    const chips = stateChips(object, live, width, (height - 22) / 2);
    const right = chips.length > 0 ? Math.min(...chips.map((chip2) => Number(chip2.children[0].attrs.x))) - 8 : width - INSET2;
    const numberSize = Math.min(22, fontSize(height, 0.42));
    const unit = object.unit ?? object.bind.value?.unit;
    const shown = live?.state === "unknown" ? "?" : live ? formatValue(live.value, object) : object.text ?? "";
    const numberWidth = textWidth(shown, numberSize, 600) + (live && live.state !== "unknown" && unit ? 4 + textWidth(unit, 14, 500) : 0);
    const label = fitWidth(object.label ?? "", right - numberWidth - 8 - INSET2, 14, 500);
    const children = [
      card(width, height, live),
      text(label, { x: INSET2, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 500 })
    ];
    const number2 = {
      x: right,
      y: baseline,
      "font-size": numberSize,
      "font-weight": 600,
      "text-anchor": "end",
      "font-variant-numeric": "tabular-nums"
    };
    if (live?.state === "unknown") {
      children.push(hatch(width, height));
      children.push(text("?", { ...number2, fill: token("unknown") }));
    } else if (live) {
      children.push(
        run({ ...number2, fill: token("text") }, [
          tspan(shown),
          ...unit ? [tspan(unit, { dx: 4, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : []
        ])
      );
    }
    if (!live && object.text) {
      children.push(text(fitWidth(object.text, right - INSET2, numberSize, 600), { ...number2, fill: token("text") }));
    }
    children.push(...chips);
    return el("g", { class: "hmi-object hmi-value hmi-value-compact", ...stateAttributes(live) }, children);
  }

  // src/hmiBuilder/drawing/components/index.ts
  var DRAWERS = {
    "alarm-list": drawAlarmList,
    bar: drawBar,
    button: drawButton,
    "event-log": drawEventLog,
    gauge: drawGauge,
    group: drawGroup,
    image: drawImage,
    junction: drawJunction,
    line: drawLine,
    "mode-selector": drawModeSelector,
    "numeric-input": drawSetpoint,
    "page-button": drawPageButton,
    panel: drawPanel,
    "pulse-button": drawButton,
    selector: drawSelector,
    setpoint: drawSetpoint,
    slider: drawSlider,
    "split-bar": drawSplitBar,
    status: drawStatus,
    switch: drawSwitch,
    table: drawTable,
    text: drawText,
    trend: drawTrend,
    value: drawValue,
    // Every symbol is drawn as equipment (§14.3).
    ...Object.fromEntries(Object.keys(SYMBOLS).map((type) => [type, drawEquipment]))
  };
  function drawObject(object, frame) {
    const draw = DRAWERS[object.type] ?? (symbolFor(object.type) ? drawEquipment : void 0);
    return draw ? draw(object, frame) : drawPlaceholder(object);
  }

  // src/hmiBuilder/drawing/placed.ts
  var PLACED_FACEPLATE = "faceplate";
  var PLACED_HEADER = 44;
  function drawPlaced(object, frame, content) {
    const [width, height] = object.size;
    const placement = frame.placed?.[object.id];
    if (placement === null) return missing(object, true);
    if (placement === void 0) return missing(object, false);
    const { model } = placement;
    const [faceplateWidth, faceplateHeight] = model.size;
    const inner = {
      ...frame,
      size: model.size,
      commands: placement.commands,
      drafts: placement.drafts,
      locked: placement.locked,
      placed: void 0,
      trendViews: void 0,
      problems: void 0,
      warnings: void 0
    };
    const total = faceplateHeight + PLACED_HEADER;
    return el("g", { class: "hmi-object hmi-placed" }, [
      el(
        "svg",
        {
          "data-placed": object.id,
          width,
          height,
          viewBox: `0 0 ${faceplateWidth} ${total}`,
          preserveAspectRatio: "xMidYMid meet",
          overflow: "visible"
        },
        [
          card(faceplateWidth, total, void 0, 16),
          header(model.title, model.description !== void 0 ? translate(model.description) : void 0, faceplateWidth),
          // The panel's own labels and buttons in the display's language, as a page's are (§14.5).
          el("g", { transform: `translate(0 ${PLACED_HEADER})` }, content(translatedPage(model), inner))
        ]
      )
    ]);
  }
  function header(title, description, width) {
    const shownTitle = fitWidth(title, width - 36, 16, 650);
    const children = [
      text(shownTitle, { x: 18, y: 28, fill: token("text"), "font-size": 16, "font-weight": 650 }),
      el("line", { x1: 0, y1: PLACED_HEADER, x2: width, y2: PLACED_HEADER, stroke: token("border") })
    ];
    const after = 18 + textWidth(shownTitle, 16, 650) + 12;
    const room = width - after - 18;
    if (description && room >= 48) {
      children.push(
        text(fitWidth(description, room, 14, 500), {
          x: after,
          y: 28,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    return el("g", { "data-part": "placed-header" }, children);
  }
  function missing(object, lacking) {
    const [width, height] = object.size;
    const tag = object.tag ?? object.instance?.split(".").pop() ?? "";
    const children = [card(width, height, void 0, 16), header(tag, void 0, width)];
    if (lacking) {
      const notice = say("No faceplate {name}", { name: object.faceplate ?? "" });
      children.push(
        text(fitWidth(notice, width - 36, 14, 500), {
          x: width / 2,
          y: PLACED_HEADER + (height - PLACED_HEADER) / 2 + 5,
          "text-anchor": "middle",
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500
        })
      );
    }
    return el("g", { class: "hmi-object hmi-placed" }, children);
  }

  // src/hmiBuilder/drawing/index.ts
  function drawPage(page, frame) {
    return withSpeech(frame, () => drawSpoken(translatedPage(page), frame));
  }
  function drawSpoken(page, frame) {
    const [width, height] = frame.size;
    return el(
      "svg",
      { xmlns: "http://www.w3.org/2000/svg", class: "hmi-page", viewBox: `0 0 ${width} ${height}`, width, height },
      [
        definitions(),
        ...backdrop(width, height),
        ...page.regions?.length ? drawRegions(page, frame) : drawContent(page, frame)
      ]
    );
  }
  function drawRegions(page, frame) {
    return (page.regions ?? []).map((region) => {
      const ids = new Set(region.objects);
      const part = {
        ...page,
        regions: void 0,
        object: page.object.filter((object) => ids.has(object.id)).map((object) => {
          if (!object.attach?.startsWith("pipe:")) return object;
          return { ...object, attach: `pipe:${region.pipes.indexOf(Number(object.attach.slice(5)))}` };
        }),
        pipe: region.pipes.map((index) => page.pipe[index])
      };
      return el(
        "svg",
        {
          x: region.at[0],
          y: region.at[1],
          width: region.size[0],
          height: region.size[1],
          viewBox: `0 0 ${region.size[0]} ${region.size[1]}`,
          overflow: "hidden",
          "data-shared-panel": region.panel ?? void 0
        },
        [
          ...region.panel ? [el("rect", { width: region.size[0], height: region.size[1], fill: token("surface") })] : [],
          ...drawContent(part, { ...frame, size: region.size })
        ]
      );
    });
  }
  function drawContent(page, frame) {
    const draw = (object) => hiddenNow(object, frame) ? el("g", { "data-object": object.id, "data-hidden": "true" }) : drawingObject(
      object.id,
      () => placed2(
        object,
        flagged(object, equipped(page, object, marked(object, drawShown(object, frame), frame), frame), frame)
      )
    );
    return [
      // A layout panel is a background, such as a room the process runs through: the pipes cross it.
      ...page.object.filter((object) => object.type === "panel").map(draw),
      // The pipes and the instruments' lines under the other objects, so a symbol lies over their
      // ends (§6).
      ...drawPipes(page, frame),
      ...drawAttachments(page, frame),
      ...drawSignals(page, frame),
      ...page.object.filter((object) => object.type !== "panel").map(draw)
    ];
  }
  function drawShown(object, frame) {
    if (object.type === PLACED_FACEPLATE) return drawPlaced(object, frame, drawContent);
    const shown = object.type === "trend" || object.type === "split-bar" || object.type.startsWith("my:") ? object : withValueFormat(object, frame);
    const drawn2 = drawObject(shown, frame);
    if (frame.currentPage && object.click?.page === frame.currentPage) {
      return {
        ...drawn2,
        attrs: { ...drawn2.attrs, "aria-current": "page" },
        children: [
          el("rect", {
            x: 0,
            y: 0,
            width: object.size[0],
            height: object.size[1],
            rx: 8,
            fill: token("accent"),
            "fill-opacity": 0.15
          }),
          ...drawn2.children,
          el("rect", { x: 0, y: 4, width: 3, height: Math.max(0, object.size[1] - 8), rx: 1.5, fill: token("accent") })
        ]
      };
    }
    return drawn2;
  }
  function hiddenNow(object, frame) {
    if (!object.bind.visible) return false;
    const live = liveValueOf(object, "visible", frame);
    return live?.state !== "unknown" && live?.value === false;
  }
  function marked(object, drawn2, frame) {
    const missing2 = frame.missing ?? [];
    if (!Object.values(object.bind).some((binding) => missing2.includes(binding.path))) return drawn2;
    const [width, height] = object.size;
    const label = say("Not in running program");
    const labelWidth = Math.round(textWidth(label, 14, 600) + 16);
    const top = Math.max(4, height - 30);
    const mark = el("g", { "data-part": "missing" }, [
      el("rect", {
        x: 1,
        y: 1,
        width: width - 2,
        height: height - 2,
        rx: 12,
        fill: "none",
        stroke: token("warning"),
        "stroke-width": 1.5,
        "stroke-dasharray": "6 4"
      }),
      el("rect", {
        x: (width - labelWidth) / 2,
        y: top,
        width: labelWidth,
        height: 24,
        rx: 12,
        fill: token("raised"),
        stroke: token("warning"),
        "stroke-opacity": 0.6
      }),
      text(label, {
        x: width / 2,
        y: top + 17,
        fill: token("warning"),
        "font-size": 14,
        "font-weight": 600,
        "text-anchor": "middle"
      })
    ]);
    return { ...drawn2, attrs: { ...drawn2.attrs, "data-missing": "true" }, children: [...drawn2.children, mark] };
  }
  function flagged(object, drawn2, frame) {
    const problems = frame.problems?.[object.id] ?? [];
    const warnings = frame.warnings?.[object.id] ?? [];
    if (problems.length === 0 && warnings.length === 0) return drawn2;
    const [width, height] = object.size;
    const sign = (colour) => [
      el("circle", { cx: 0, cy: 0, r: 9, fill: token(colour), stroke: token("page"), "stroke-width": 2 }),
      text("!", { x: 0, y: 4.5, fill: token("page"), "font-size": 12, "font-weight": 800, "text-anchor": "middle" })
    ];
    const mark = problems.length > 0 ? el("g", { "data-part": "problem" }, [
      el("rect", {
        x: 1,
        y: 1,
        width: Math.max(0, width - 2),
        height: Math.max(0, height - 2),
        rx: 12,
        fill: "none",
        stroke: token("alarm"),
        "stroke-width": 2,
        "stroke-dasharray": "6 4"
      }),
      ...sign("alarm")
    ]) : el("g", { "data-part": "warning" }, sign("warning"));
    const title = { ...el("title"), text: [...problems, ...warnings].join("\n") };
    const attrs = {
      ...drawn2.attrs,
      ...problems.length > 0 ? { "data-problems": "true" } : {},
      ...warnings.length > 0 ? { "data-warnings": "true" } : {}
    };
    return { ...drawn2, attrs, children: [title, ...drawn2.children, mark] };
  }
  function equipped(page, object, drawn2, frame) {
    if (page.fields !== void 0 && !(object.type in SYMBOLS)) return drawn2;
    const badge = equipmentBadge(object, frame);
    return badge ? { ...drawn2, children: [...drawn2.children, badge] } : drawn2;
  }
  function placed2(object, drawn2) {
    const [x, y] = object.at;
    const [width, height] = object.size;
    const transform = object.rotation ? `translate(${x} ${y}) rotate(${object.rotation} ${width / 2} ${height / 2})` : `translate(${x} ${y})`;
    const own = object.click ? Object.entries(object.click).find(([key]) => key !== "instance") : void 0;
    const click = own ?? (object.type !== "faceplate" && object.faceplate ? ["faceplate", object.faceplate] : void 0);
    const hit = el("rect", { "data-part": "hit", x: 0, y: 0, width, height, fill: "transparent" });
    return {
      ...drawn2,
      attrs: {
        ...drawn2.attrs,
        "data-object": object.id,
        transform,
        ...click ? { "data-click": `${click[0]}:${click[1]}` } : {}
      },
      children: click ? [hit, ...drawn2.children] : drawn2.children
    };
  }

  // src/hmiBuilder/drawing/tokens.ts
  var CJK_FONTS = [
    { code: "SC", family: "truST CJK SC", file: "NotoSansSC-Subset.woff2" },
    { code: "JP", family: "truST CJK JP", file: "NotoSansJP-Subset.woff2" },
    { code: "KR", family: "truST CJK KR", file: "NotoSansKR-Subset.woff2" }
  ];
  var CJK_RANGE = "U+1100-11FF, U+2E80-2FDF, U+3000-33FF, U+3400-4DBF, U+4E00-9FFF, U+A960-A97F, U+AC00-D7AF, U+F900-FAFF, U+FE30-FE4F, U+FF00-FFEF";
  function fontStack(first) {
    const cjk = first.map((code) => `"${CJK_FONTS.find((font) => font.code === code)?.family}"`).join(", ");
    return `Inter, ${cjk}, "Segoe UI Variable Display", "Segoe UI", system-ui, -apple-system, Roboto, "Helvetica Neue", Arial, sans-serif`;
  }
  var FONT = fontStack(["SC", "JP", "KR"]);
  var LANGUAGE_FONTS = [
    ["ja", fontStack(["JP", "SC", "KR"])],
    ["ko", fontStack(["KR", "JP", "SC"])]
  ];
  var TOKEN_THEMES = {
    dark: {
      page: "#06080c",
      surface: "#0d1117",
      raised: "#161c26",
      border: "#1d2430",
      line: "#1a212c",
      text: "#eceff4",
      muted: "#8a94a8",
      accent: "#22d3ee",
      "accent-2": "#6366f1",
      "accent-3": "#a855f7",
      "pen-1": "#22d3ee",
      "pen-2": "#a3e635",
      "pen-3": "#818cf8",
      "pen-4": "#e2e8f0",
      unknown: "#667085",
      stale: "#e3b24f",
      pending: "#a78bfa",
      warning: "#fbbf24",
      alarm: "#fb4f6e",
      "alarm-high": "#fb4f6e",
      "alarm-medium": "#fb923c",
      "alarm-low": "#facc15",
      font: FONT
    },
    light: {
      page: "#f3f5f9",
      surface: "#ffffff",
      raised: "#eef1f6",
      border: "#e3e8ef",
      line: "#e6eaf1",
      text: "#0b1220",
      muted: "#475569",
      accent: "#0e7490",
      "accent-2": "#4f46e5",
      "accent-3": "#9333ea",
      "pen-1": "#0e7490",
      "pen-2": "#4d7c0f",
      "pen-3": "#4f46e5",
      "pen-4": "#334155",
      unknown: "#64748b",
      stale: "#92400e",
      pending: "#5b21b6",
      warning: "#92400e",
      alarm: "#be123c",
      "alarm-high": "#be123c",
      "alarm-medium": "#ea580c",
      "alarm-low": "#a16207",
      font: FONT
    }
  };
  var HEX_COLOUR = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;
  function fontFaces(fontUrl) {
    const folder = fontUrl.slice(0, fontUrl.lastIndexOf("/") + 1);
    const face = (family, url, range2) => `@font-face {
  font-family: "${family}";
  src: url("${url}") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
${range2 ? `  unicode-range: ${range2};
` : ""}}
`;
    return face("Inter", fontUrl) + CJK_FONTS.map((font) => face(font.family, `${folder}${font.file}`, CJK_RANGE)).join("");
  }
  var SAFE_URL = /^[^"'()\\\s<>]+$/;
  function tokensCss(colors = {}, fontUrl) {
    const themes = Object.entries(TOKEN_THEMES).map(([theme, defaults]) => {
      const tokens = { ...defaults };
      for (const [name, value] of Object.entries(colors[theme] ?? {})) {
        if (name !== "font" && name in defaults && HEX_COLOUR.test(value)) tokens[name] = value;
      }
      const declarations = Object.entries(tokens).map(([name, value]) => `  --hmi-${name}: ${value};`).join("\n");
      return `.hmi-theme-${theme} {
${declarations}
  background: var(--hmi-page);
  color-scheme: ${theme};
}`;
    });
    const fonts = fontUrl !== void 0 && SAFE_URL.test(fontUrl) ? fontFaces(fontUrl) : "";
    const languages = LANGUAGE_FONTS.map(
      ([language2, stack]) => `:is(.hmi-theme-dark, .hmi-theme-light):lang(${language2}) {
  --hmi-font: ${stack};
}`
    ).join("\n");
    return `${fonts}${themes.join("\n")}
${languages}
.hmi-page { font-family: var(--hmi-font); font-optical-sizing: none; }
${MOTION_CSS}`;
  }
  var MOTION_CSS = `@keyframes hmi-spin { to { transform: rotate(360deg); } }
.hmi-spin { transform-box: fill-box; transform-origin: center; animation: hmi-spin ${SPIN_PERIOD_MS}ms linear infinite; }
@keyframes hmi-stroke { to { transform: translateX(8px); } }
.hmi-stroke { animation: hmi-stroke ${SPIN_PERIOD_MS / 2}ms ease-in-out infinite alternate; }
@keyframes hmi-flow { to { stroke-dashoffset: -16; } }
.hmi-flow { animation: hmi-flow ${FLOW_PERIOD_MS}ms linear infinite; }
.hmi-flow-back { animation: hmi-flow ${FLOW_PERIOD_MS}ms linear infinite reverse; }
@media (prefers-reduced-motion: reduce) { .hmi-spin { animation: none; } .hmi-stroke, .hmi-flow, .hmi-flow-back { animation: none; } }
`;

  // src/hmiBuilder/editor/designSamples.ts
  function designValues(page, catalogue) {
    var _a;
    const values = {};
    const ranged = /* @__PURE__ */ new Set();
    for (const object of page.object) {
      const component = catalogue.components.find((candidate) => candidate.type === object.type);
      const symbol = SYMBOLS[object.type];
      const shown = symbol?.states.find((state5) => state5.id === symbol.sample)?.values ?? {};
      for (const [property, binding] of Object.entries(object.bind)) {
        const accepts = component?.properties?.[property]?.accepts ?? component?.additional?.accepts ?? ["value"];
        const state5 = component?.category === "equipment" && accepts[0] === "boolean";
        const field = binding.path.startsWith("$") ? page.fields?.[binding.path.slice(1)]?.accepts[0] : void 0;
        const value = property in shown ? resolveState(shown[property], object) : state5 ? EQUIPMENT_NORMAL.includes(property) : sample(
          binding.range ? { ...object, min: binding.range[0], max: binding.range[1] } : object,
          binding.path,
          field && field !== "value" ? field : accepts[0]
        );
        const explicitRange = binding.range !== void 0 || object.min !== void 0 || object.max !== void 0;
        if (typeof value === "number" && explicitRange && !ranged.has(binding.path)) {
          values[binding.path] = { value, state: "confirmed" };
          ranged.add(binding.path);
        } else values[_a = binding.path] ?? (values[_a] = { value, state: "confirmed" });
      }
    }
    return values;
  }
  var EQUIPMENT_NORMAL = ["running", "open"];
  function sample(object, path, kind) {
    if (object.type === "selector" || object.type === "mode-selector")
      return object.options?.[0] ?? (object.type === "mode-selector" ? "Auto" : 0);
    if (object.type === "text" || kind === "enumeration" || kind === "text")
      return path.split(".").pop()?.replace(/^\$/, "") ?? path;
    if (kind === "boolean") return true;
    if (kind === "array" || kind === "structure") return [];
    const min = object.min ?? 0;
    const max = object.max ?? 100;
    return Number((min + (max - min) * 0.6).toFixed(6));
  }
  return __toCommonJS(runtimeBundle_exports);
})();
