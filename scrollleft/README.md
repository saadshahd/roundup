# .columns scrollLeft per animation frame, U111

Harness (`just harness tree-40`), headless Chromium, requestAnimationFrame sampling over 900 ms per open and close. main f0a5e66 vs PR head 9165562. Raw frames: frames.csv.

| build | window w | drawer | phase | frames | max scrollLeft px | frames with scrollLeft > 0 | drawer left px (first, min, last) |
|---|---|---|---|---|---|---|---|
| main f0a5e66 | 800 | pad | open | 57 | 548 | 12 | 800..252..252 |
| main f0a5e66 | 800 | pad | close | 57 | 0 | 0 | 252..252..800 |
| main f0a5e66 | 800 | todo | open | 57 | 548 | 11 | 800..252..252 |
| main f0a5e66 | 800 | todo | close | 57 | 0 | 0 | 252..252..800 |
| main f0a5e66 | 1600 | pad | open | 57 | 560 | 12 | 1600..1040..1040 |
| main f0a5e66 | 1600 | pad | close | 57 | 0 | 0 | 1040..1040..1600 |
| main f0a5e66 | 1600 | todo | open | 57 | 560 | 11 | 1600..1040..1040 |
| main f0a5e66 | 1600 | todo | close | 57 | 0 | 0 | 1040..1040..1600 |
| #224 9165562 | 800 | pad | open | 57 | 0 | 0 | 800..252..252 |
| #224 9165562 | 800 | pad | close | 57 | 0 | 0 | 252..252..800 |
| #224 9165562 | 800 | todo | open | 57 | 0 | 0 | 800..252..252 |
| #224 9165562 | 800 | todo | close | 57 | 0 | 0 | 252..252..800 |
| #224 9165562 | 1600 | pad | open | 57 | 0 | 0 | 1600..1040..1040 |
| #224 9165562 | 1600 | pad | close | 57 | 0 | 0 | 1040..1040..1600 |
| #224 9165562 | 1600 | todo | open | 57 | 0 | 0 | 1600..1040..1040 |
| #224 9165562 | 1600 | todo | close | 57 | 0 | 0 | 1040..1040..1600 |

Chromium resets main's scrollLeft to 0 once the slide ends. The reported stuck state is WKWebView and is not covered here.
