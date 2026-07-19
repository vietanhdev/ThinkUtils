import { defineConfig } from 'vitepress';
import { withMermaid } from 'vitepress-plugin-mermaid';

export default withMermaid(
  defineConfig({
    title: 'ThinkUtils',
    description:
      'A powerful, native desktop application that unlocks the full potential of your ThinkPad on Linux.',
    head: [['link', { rel: 'icon', href: '/favicon.ico' }]],
    themeConfig: {
      logo: '/logo.svg',
      nav: [
        { text: 'Guide', link: '/guide/getting-started' },
        { text: 'Development', link: '/development/architecture' },
        { text: 'Download', link: '/download' },
      ],
      sidebar: {
        '/guide/': [
          {
            text: 'Introduction',
            items: [
              { text: 'Getting Started', link: '/guide/getting-started' },
              { text: 'Installation', link: '/guide/installation' },
              { text: 'Permissions', link: '/guide/permissions' },
            ],
          },
          {
            text: 'Features',
            items: [
              { text: 'Fan Control', link: '/guide/fan-control' },
              { text: 'Battery Management', link: '/guide/battery' },
              { text: 'Performance Tuning', link: '/guide/performance' },
              { text: 'System Monitor', link: '/guide/monitor' },
              { text: 'Security', link: '/guide/security' },
              { text: 'AI Integration (MCP)', link: '/guide/mcp' },
              { text: 'Settings Sync', link: '/guide/sync' },
            ],
          },
        ],
        '/development/': [
          {
            text: 'Development',
            items: [
              { text: 'Architecture', link: '/development/architecture' },
              { text: 'CSS Architecture', link: '/development/css' },
              { text: 'Icon Generation', link: '/development/icons' },
              {
                text: 'Google OAuth Setup',
                link: '/development/google-oauth',
              },
            ],
          },
        ],
      },
      socialLinks: [
        { icon: 'github', link: 'https://github.com/vietanhdev/ThinkUtils' },
      ],
      footer: {
        message: 'Released under the LGPL v3 License.',
        copyright: 'Copyright © Viet Anh Nguyen',
      },
      search: {
        provider: 'local',
      },
    },
    mermaid: {
      theme: 'default',
    },
    mermaidPlugin: {
      class: 'mermaid',
    },
  }),
);
