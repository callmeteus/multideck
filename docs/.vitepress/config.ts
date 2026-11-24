import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'en-US',
  title: 'MultiDeck',
  description:
    'Stream deck software for Windows and Linux. One profile format and one device adapter layer for every deck.',
  cleanUrls: true,
  lastUpdated: true,
  appearance: 'dark',
  srcExclude: ['research/**'],
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/logo-mark.svg' }],
    ['meta', { name: 'theme-color', content: '#0e1014' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'MultiDeck' }],
    [
      'meta',
      {
        property: 'og:description',
        content: 'One deck, any device. Stream deck software for Windows and Linux.',
      },
    ],
    ['meta', { property: 'og:image', content: '/og-cover.svg' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
  ],
  themeConfig: {
    logo: '/logo-mark.svg',
    siteTitle: 'MultiDeck',
    nav: [
      { text: 'Guide', link: '/guide/getting-started', activeMatch: '/guide/' },
      { text: 'Architecture', link: '/architecture' },
      { text: 'Development', link: '/development' },
      { text: 'Roadmap', link: '/roadmap' },
    ],
    sidebar: [
      {
        text: 'Introduction',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Getting started', link: '/guide/getting-started' },
        ],
      },
      {
        text: 'Guide',
        items: [
          { text: 'Devices', link: '/guide/devices' },
          { text: 'Profiles and pages', link: '/guide/profiles' },
          { text: 'Actions', link: '/guide/actions' },
          { text: 'Language', link: '/guide/language' },
        ],
      },
      {
        text: 'Project',
        items: [
          { text: 'Architecture', link: '/architecture' },
          { text: 'Development', link: '/development' },
          { text: 'Roadmap', link: '/roadmap' },
        ],
      },
    ],
    search: {
      provider: 'local',
    },
    socialLinks: [{ icon: 'github', link: 'https://github.com/callmeteus/multideck' }],
    editLink: {
      pattern: 'https://github.com/callmeteus/multideck/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
    outline: {
      level: [2, 3],
      label: 'On this page',
    },
    docFooter: {
      prev: 'Previous',
      next: 'Next',
    },
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright 2026 MultiDeck contributors',
    },
    lastUpdated: {
      text: 'Last updated',
      formatOptions: { dateStyle: 'medium' },
    },
  },
})
