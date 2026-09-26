<script lang="ts">
  // Minimal stroke icon set (24x24).
  let { name, size = 18 }: { name: string; size?: number } = $props();

  const paths: Record<string, string> = {
    hand: "M8 13V5.5a1.5 1.5 0 0 1 3 0V11m0-6.5V4a1.5 1.5 0 0 1 3 0v7m0-5.5a1.5 1.5 0 0 1 3 0V12m0-4a1.5 1.5 0 0 1 3 0v6a7 7 0 0 1-7 7h-1a7 7 0 0 1-5.6-2.8L4.3 15a1.6 1.6 0 0 1 2.5-2L8 14.5",
    mirror: "M12 3v18M9 6 3 18h6zM15 6l6 12h-6z",
    lock: "M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4",
    unlock: "M6 11h12v10H6zM8 11V7a4 4 0 0 1 7.5-2",
    power: "M12 3v8M6.3 6.3a8 8 0 1 0 11.4 0",
    camera: "M4 8h3l2-3h6l2 3h3v11H4zM12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
    "rotate-ccw": "M4 4v5h5M4.6 13a8 8 0 1 0 1.8-6.4L4 9",
    "rotate-cw": "M20 4v5h-5M19.4 13a8 8 0 1 1-1.8-6.4L20 9",
    flip: "M12 3v18M4 17 10 7v10zM20 17l-6-10v10",
    person: "M12 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM5 20a7 7 0 0 1 14 0",
    group: "M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM3 19a6 6 0 0 1 12 0M16 11a2.5 2.5 0 1 0 0-5M17 13.5a5 5 0 0 1 4 5",
    "chevron-down": "m6 9 6 6 6-6",
    "chevron-up": "m6 15 6-6 6 6",
    reset: "M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5",
    help: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17.5v.01",
    video: "M3 7h12v10H3zM15 10l6-3v10l-6-3",
    refresh: "M20 11a8 8 0 0 0-14.5-4M4 4v4h4M4 13a8 8 0 0 0 14.5 4M20 20v-4h-4",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? ""} />
</svg>
