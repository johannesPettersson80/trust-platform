/* The firmware's memory routines (src/memcpy.rs, src/memset.rs, src/memcmp.rs) case by case under
   qemu-riscv32, against what C gives (specification 34 D.2). Exit 0 when every case holds, else the
   number of the first case that fails. The ROM's routines, which they call for long ranges, are
   plain loops here; the ROM's memcpy copies from the first byte to the last. */
typedef unsigned char u8;
typedef unsigned int u32;

void *trust_memcpy(void *dst, const void *src, u32 n);
void *trust_memset(void *dst, int c, u32 n);
int trust_memcmp(const void *a, const void *b, u32 n);

/* The ROM's routines, and the C library's that the compiler may call for the loops below: volatile,
   so that the compiler does not make them calls of themselves. */
void *rom_memcpy(void *d, const void *s, u32 n) {
  volatile u8 *p = d;
  const volatile u8 *q = s;
  while (n--) *p++ = *q++;
  return d;
}
void *rom_memset(void *d, int c, u32 n) {
  volatile u8 *p = d;
  while (n--) *p++ = (u8)c;
  return d;
}
int rom_memcmp(const void *a, const void *b, u32 n) {
  const volatile u8 *p = a, *q = b;
  for (; n; n--, p++, q++)
    if (*p != *q) return *p - *q;
  return 0;
}
void *memcpy(void *d, const void *s, u32 n) { return rom_memcpy(d, s, n); }
void *memset(void *d, int c, u32 n) { return rom_memset(d, c, n); }
void *memmove(void *d, const void *s, u32 n) {
  volatile u8 *p = d;
  const volatile u8 *q = s;
  if (p < q) {
    while (n--) *p++ = *q++;
  } else {
    p += n;
    q += n;
    while (n--) *--p = *--q;
  }
  return d;
}

static u8 src[256] __attribute__((aligned(8)));
static u8 dst[288] __attribute__((aligned(8)));
static u8 ref[288] __attribute__((aligned(8)));

static void finish(int code) {
  register int a0 __asm__("a0") = code;
  register int a7 __asm__("a7") = 93; /* exit */
  __asm__ volatile("ecall" ::"r"(a0), "r"(a7));
  for (;;) {
  }
}

/* Every size 0..160 at every pair of offsets within a double word, the ranges apart: the bytes
   copied, the destination returned, and no byte written outside the destination. */
static void copies_apart(void) {
  for (u32 n = 0; n <= 160; n++)
    for (u32 so = 0; so < 8; so++)
      for (u32 dof = 0; dof < 8; dof++) {
        for (u32 i = 0; i < sizeof src; i++) src[i] = (u8)(i * 13 + 7);
        for (u32 i = 0; i < sizeof dst; i++) dst[i] = 0x5A;
        if (trust_memcpy(dst + 16 + dof, src + so, n) != dst + 16 + dof) finish(1);
        for (u32 i = 0; i < n; i++)
          if (dst[16 + dof + i] != src[so + i]) finish(2);
        for (u32 i = 0; i < 16 + dof; i++)
          if (dst[i] != 0x5A) finish(3);
        for (u32 i = 16 + dof + n; i < sizeof dst; i++)
          if (dst[i] != 0x5A) finish(4);
      }
}

/* A copy to a lower address that overlaps its source, which C leaves undefined but the Wi-Fi
   driver's closed libraries make: it gives what a copy from the first byte to the last gives, as the
   ROM's does. The routine's word table once ran from the last word to the first, and every WPA2
   handshake failed at its third message (checklist L3, 25 October 2026). */
static void copies_down_over_their_source(void) {
  static const u32 shifts[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 16, 24, 63};
  for (u32 k = 0; k < sizeof shifts / sizeof shifts[0]; k++)
    for (u32 n = 0; n <= 160; n++)
      for (u32 off = 0; off < 8; off++) {
        u32 d = 16 + off, s = d + shifts[k];
        for (u32 i = 0; i < sizeof dst; i++) {
          dst[i] = (u8)(i * 7 + 3);
          ref[i] = dst[i];
        }
        for (u32 i = 0; i < n; i++) ref[d + i] = ref[s + i];
        if (trust_memcpy(dst + d, dst + s, n) != dst + d) finish(5);
        for (u32 i = 0; i < sizeof dst; i++)
          if (dst[i] != ref[i]) finish(6);
      }
}

/* Every size 0..160 at every destination offset 0..7 with three fill values, one above a byte: the
   bytes filled with the value's low byte, the destination returned, and nothing else written. */
static void fills(void) {
  static const int values[3] = {0x00, 0xA5, 0x1FF};
  for (u32 f = 0; f < 3; f++)
    for (u32 n = 0; n <= 160; n++)
      for (u32 off = 0; off < 8; off++) {
        for (u32 i = 0; i < sizeof dst; i++) dst[i] = 0x5A;
        if (trust_memset(dst + 16 + off, values[f], n) != dst + 16 + off) finish(11);
        for (u32 i = 0; i < n; i++)
          if (dst[16 + off + i] != (u8)values[f]) finish(12);
        for (u32 i = 0; i < 16 + off; i++)
          if (dst[i] != 0x5A) finish(13);
        for (u32 i = 16 + off + n; i < sizeof dst; i++)
          if (dst[i] != 0x5A) finish(14);
      }
}

/* Every size 0..48 at every pair of offsets 0..3: equal ranges compare equal, and one byte greater
   or less at any position gives the sign of that byte's difference, both ways round. */
static void comparisons(void) {
  for (u32 n = 0; n <= 48; n++)
    for (u32 ao = 0; ao < 4; ao++)
      for (u32 bo = 0; bo < 4; bo++) {
        for (u32 i = 0; i < 128; i++) {
          src[i] = (u8)(i * 13 + 7);
          ref[i] = (u8)(i * 13 + 7);
        }
        for (u32 i = 0; i < n; i++) ref[bo + i] = src[ao + i];
        if (trust_memcmp(src + ao, ref + bo, n) != 0) finish(21);
        for (u32 at = 0; at < n; at++) {
          u8 saved = ref[bo + at];
          ref[bo + at] = (u8)(saved + 1);
          if (trust_memcmp(src + ao, ref + bo, n) >= 0) finish(22);
          if (trust_memcmp(ref + bo, src + ao, n) <= 0) finish(23);
          ref[bo + at] = (u8)(saved - 1);
          int r = trust_memcmp(src + ao, ref + bo, n);
          if ((saved == 0 && r >= 0) || (saved != 0 && r <= 0)) finish(24);
          ref[bo + at] = saved;
        }
      }
}

void _start(void) {
  copies_apart();
  copies_down_over_their_source();
  fills();
  comparisons();
  finish(0);
}
