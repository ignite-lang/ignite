function median(a, n,   i, j, t) {
  for (i = 2; i <= n; i++) { t = a[i]; j = i - 1; while (j > 0 && a[j] > t) { a[j + 1] = a[j]; j-- } a[j + 1] = t }
  return (n % 2) ? a[(n + 1) / 2] : (a[n / 2] + a[n / 2 + 1]) / 2
}
$1 == "STEP" { key = $2 SUBSEP $4; sec[$2, $3, $4] = $5; load[$2, $3, $4] = $6; ex[$2, $3, $4] = $7; units[$2] = 1; runs[$2, $4] = 1 }
END {
  for (u in units) {
    nb = 0; na = 0; nr = 0; drop = 0; lo = 1e9; hi = 0
    for (r = 1; r <= 15; r++) {
      if (!((u, "before", r) in sec) && !((u, "after", r) in sec)) continue
      if (!((u, "before", r) in sec)) { a1[++na] = sec[u, "after", r]; l = load[u, "after", r]; if (l < lo) lo = l; if (l > hi) hi = l; continue }
      if (ex[u, "before", r] != 0 || ex[u, "after", r] != 0) { drop++; continue }
      d = load[u, "before", r] - load[u, "after", r]; if (d < 0) d = -d
      if (d > 5) { drop++; continue }
      b[++nb] = sec[u, "before", r]; a2[nb] = sec[u, "after", r]; rt[++nr] = sec[u, "after", r] / sec[u, "before", r]
      for (s = 0; s < 2; s++) { l = load[u, s ? "after" : "before", r]; if (l < lo) lo = l; if (l > hi) hi = l }
    }
    if (nb > 0) printf "%s | %.3f | %.3f | %.2f | %d/%d | %.1f to %.1f\n", u, median(b, nb), median(a2, nb), median(rt, nr), nb, drop, lo, hi
    else if (na > 0) printf "%s | | %.3f | | %d runs | %.1f to %.1f\n", u, median(a1, na), na, lo, hi
    delete b; delete a2; delete rt; delete a1
  }
}
