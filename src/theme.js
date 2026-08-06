// 共享主题系统：按月变化的预设主题 + 自定义主题。
// 色板取自 VineTurtleStudio/CSS/style.css（body.month-N 变量）。
window.Theme = (function () {
    'use strict';

    const MONTH_THEMES = [
        { primary: '#5b6e7a', secondary: '#a3b6c4', bg: '#edf2f5', accent: '#7e94a0', text: '#354952' }, // 1 月
        { primary: '#6b4e3a', secondary: '#b28c73', bg: '#f5ede8', accent: '#8c6c56', text: '#4a362a' }, // 2 月
        { primary: '#7eb36a', secondary: '#c8dfa4', bg: '#f7faf2', accent: '#9dcf86', text: '#3f5c34' }, // 3 月
        { primary: '#d9928a', secondary: '#f0cdc8', bg: '#fdf6f5', accent: '#e5b2ab', text: '#775954' }, // 4 月
        { primary: '#2a5c3a', secondary: '#88b17e', bg: '#e4efe1', accent: '#4a7c5b', text: '#23382c' }, // 5 月
        { primary: '#5a8c4e', secondary: '#f1a35c', bg: '#f3f8eb', accent: '#8ab06b', text: '#374d2e' }, // 6 月
        { primary: '#7b7a9c', secondary: '#c2c0d9', bg: '#f2f0f8', accent: '#a09fc0', text: '#484764' }, // 7 月
        { primary: '#bca45c', secondary: '#decb8c', bg: '#fcf8ee', accent: '#cbb874', text: '#635632' }, // 8 月
        { primary: '#9a4c4c', secondary: '#c68686', bg: '#f5edec', accent: '#b86969', text: '#5e3232' }, // 9 月
        { primary: '#b85a32', secondary: '#e69b6a', bg: '#fcf3eb', accent: '#d07b4c', text: '#6a3822' }, // 10 月
        { primary: '#708090', secondary: '#b0bec5', bg: '#f0f2f4', accent: '#8da0ad', text: '#455560' }, // 11 月
        { primary: '#1b3b29', secondary: '#4a6b5a', bg: '#e0e7e1', accent: '#335543', text: '#1a2b20' }, // 12 月
    ];

    function hexToRgb(hex) {
        const h = String(hex || '#000000').replace('#', '');
        const v = h.length === 3 ? h.split('').map(c => c + c).join('') : h;
        return [0, 2, 4].map(i => parseInt(v.slice(i, i + 2), 16));
    }
    function clamp(n) { return Math.max(0, Math.min(255, Math.round(n))); }
    function mix(a, b, ratio) {
        const A = hexToRgb(a), B = hexToRgb(b);
        const c = A.map((v, i) => clamp(v + (B[i] - v) * ratio));
        return '#' + c.map(x => x.toString(16).padStart(2, '0')).join('');
    }
    function darken(c, ratio) {
        const out = hexToRgb(c).map(v => clamp(v * (1 - ratio)));
        return '#' + out.map(x => x.toString(16).padStart(2, '0')).join('');
    }

    // 将一个月/自定义色板映射到应用 CSS 变量
    function paletteToApp(p) {
        return {
            bg: p.bg,
            bgSecondary: mix(p.bg, p.secondary, 0.45),
            text: p.text,
            textSecondary: mix(p.text, p.bg, 0.5),
            accent: p.accent,
            accentHover: darken(p.accent, 0.12),
            border: 'rgba(0,0,0,0.08)',
            cardBg: 'rgba(255,255,255,0.55)',
            coverBg: `linear-gradient(145deg, ${mix(p.bg, p.secondary, 0.4)}, ${mix(p.bg, p.primary, 0.28)})`
        };
    }

    function currentMonth() {
        return new Date().getMonth() + 1;
    }

    async function resolvePalette(theme) {
        const invoke = window.__TAURI__ && window.__TAURI__.core
            ? window.__TAURI__.core.invoke
            : null;
        if (theme === 'custom') {
            const c = (invoke && await invoke('store_get', { key: 'customTheme' }).catch(() => null)) || {};
            return {
                primary: c.primary || MONTH_THEMES[0].primary,
                secondary: c.secondary || MONTH_THEMES[0].secondary,
                bg: c.bg || MONTH_THEMES[0].bg,
                accent: c.accent || MONTH_THEMES[0].accent,
                text: c.text || MONTH_THEMES[0].text,
            };
        }
        const m = (typeof theme === 'string' && theme.startsWith('month-'))
            ? parseInt(theme.slice(6), 10)
            : currentMonth();
        return MONTH_THEMES[((m - 1) % 12 + 12) % 12];
    }

    async function applyTheme(theme) {
        const palette = await resolvePalette(theme);
        const colors = paletteToApp(palette);
        const root = document.documentElement;
        root.style.setProperty('--bg-primary', colors.bg);
        root.style.setProperty('--bg-secondary', colors.bgSecondary);
        root.style.setProperty('--text-primary', colors.text);
        root.style.setProperty('--text-secondary', colors.textSecondary);
        root.style.setProperty('--accent', colors.accent);
        root.style.setProperty('--accent-hover', colors.accentHover);
        root.style.setProperty('--border-color', colors.border);
        root.style.setProperty('--card-bg', colors.cardBg);
        root.style.setProperty('--cover-bg', colors.coverBg);
        return colors;
    }

    return {
        MONTH_THEMES: MONTH_THEMES,
        paletteToApp: paletteToApp,
        currentMonth: currentMonth,
        resolvePalette: resolvePalette,
        applyTheme: applyTheme,
    };
})();
