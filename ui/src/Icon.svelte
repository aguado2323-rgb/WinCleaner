<script>
  let { name, size = 18 } = $props();
  const paths = {
    broom: 'M14 3l7 7M4 20l6-6m-3.5 6.5c-2 .5-4-.5-4-.5s1-4.5 3.5-6.5l7 7c-2 2.5-4.5 3-6.5 0zM12.5 7.5l4 4',
    shield: 'M12 3l7 3v5c0 5-3 8.5-7 10-4-1.5-7-5-7-10V6l7-3z',
    disk: 'M3 7c0-1.7 4-3 9-3s9 1.3 9 3-4 3-9 3-9-1.3-9-3zm0 0v10c0 1.7 4 3 9 3s9-1.3 9-3V7M3 12c0 1.7 4 3 9 3s9-1.3 9-3',
    power: 'M12 3v8M6.3 6.8a8 8 0 1011.4 0',
    folder: 'M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z',
    undo: 'M9 14L4 9l5-5M4 9h10a6 6 0 010 12h-3',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 002 2h6a2 2 0 002-2l1-12M9 7V4h6v3',
    lock: 'M7 11V8a5 5 0 0110 0v3M6 11h12v9H6z',
    x: 'M6 6l12 12M18 6L6 18',
    chevron: 'M9 6l6 6-6 6',
    file: 'M7 3h7l5 5v13H7zM14 3v5h5',
    external: 'M14 4h6v6M20 4l-9 9M18 14v6H4V6h6',
    pie: 'M12 3a9 9 0 109 9h-9V3zM15 3.5A9 9 0 0120.5 9H15V3.5z',
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>
