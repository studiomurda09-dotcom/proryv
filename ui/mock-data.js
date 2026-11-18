// Сгенерировано dump_builtin.rs — не редактировать вручную.
window.MOCK_DATA = {
  "defaults": {
    "accent": "#7cc4e4",
    "animations": true,
    "auto_heal": true,
    "autostart": false,
    "backdrop": "acrylic",
    "bg_dim": 60,
    "bg_image": "",
    "block_quic": true,
    "check_updates": true,
    "custom_domains": [],
    "custom_strategies": [],
    "dev_mode": false,
    "doh": true,
    "doh_server": "https://cloudflare-dns.com/dns-query",
    "enable_on_launch": false,
    "enabled_presets": [
      "youtube",
      "discord",
      "general"
    ],
    "excluded_domains": [],
    "first_run_done": false,
    "game_filter": "off",
    "game_ips": [
      "128.116.0.0/17"
    ],
    "language": "system",
    "minimize_to_tray": true,
    "notifications": true,
    "random_fakes": true,
    "start_minimized": false,
    "strategy_override": [],
    "telegram": false,
    "telegram_port": 1443,
    "telegram_secret": "",
    "theme": "dark",
    "update_base": true
  },
  "presets": [
    {
      "description": "Видео, превью и мобильные клиенты YouTube.",
      "domains": [
        "youtube.com",
        "youtu.be",
        "youtube-nocookie.com",
        "youtubekids.com",
        "googlevideo.com",
        "ytimg.com",
        "ggpht.com",
        "yt3.googleusercontent.com",
        "youtubei.googleapis.com",
        "youtube.googleapis.com",
        "youtubeembeddedplayer.googleapis.com",
        "jnn-pa.googleapis.com",
        "fonts.googleapis.com",
        "www.gstatic.com",
        "play.google.com",
        "wide-youtube.l.google.com",
        "youtube-ui.l.google.com",
        "ytimg.l.google.com",
        "yt-video-upload.l.google.com",
        "google.ru"
      ],
      "icon": "youtube",
      "id": "youtube",
      "ip_id_zero": true,
      "name": "YouTube",
      "strategies": [
        "alt11",
        "fake_multidisorder",
        "alt7",
        "seqovl_google",
        "fake_ts",
        "seqovl_4pda",
        "multisplit_host",
        "faked_split_ts",
        "host_fake_split",
        "faked_disorder_ts",
        "fake_noack_split"
      ],
      "tcp_ports": [
        443
      ],
      "udp_ports": [
        [
          443,
          443
        ]
      ]
    },
    {
      "description": "Сайт, приложение, CDN и голосовые каналы (UDP).",
      "domains": [
        "discord.com",
        "discord.gg",
        "discord.app",
        "discord.co",
        "discord.design",
        "discord.dev",
        "discord.gift",
        "discord.gifts",
        "discord.media",
        "discord.new",
        "discord.store",
        "discord.status",
        "discordapp.com",
        "discordapp.net",
        "discordcdn.com",
        "discord-activities.com",
        "discordactivities.com",
        "discordmerch.com",
        "discordpartygames.com",
        "discordsays.com",
        "discordsez.com",
        "discordstatus.com",
        "dis.gd",
        "discord-attachments-uploads-prd.storage.googleapis.com",
        "stable.dl2.discordapp.net"
      ],
      "icon": "discord",
      "id": "discord",
      "ip_id_zero": false,
      "name": "Discord",
      "strategies": [
        "alt11",
        "fake_multidisorder",
        "seqovl_4pda",
        "alt7",
        "disorder_1",
        "faked_split_ts",
        "host_fake_split",
        "fake_noack_split"
      ],
      "tcp_ports": [
        443,
        2053,
        2083,
        2087,
        2096,
        8443
      ],
      "udp_ports": [
        [
          19294,
          19344
        ],
        [
          50000,
          50100
        ]
      ]
    },
    {
      "description": "Сайт, лаунчер и CDN Roblox.",
      "domains": [
        "roblox.com",
        "rbxcdn.com",
        "rbx.com",
        "robloxlabs.com"
      ],
      "icon": "gamepad",
      "id": "roblox",
      "ip_id_zero": false,
      "name": "Roblox",
      "strategies": [
        "seqovl_4pda",
        "fake_multidisorder",
        "multisplit_host",
        "faked_split_ts",
        "host_fake_split",
        "faked_disorder_ts",
        "fake_noack_split"
      ],
      "tcp_ports": [
        443
      ],
      "udp_ports": []
    },
    {
      "description": "Cloudflare, Twitch-эмоуты, DoH, трекеры, SoundCloud, Medium и др.",
      "domains": [
        "dns.google",
        "dns.quad9.net",
        "dns.nextdns.io",
        "doh.opendns.com",
        "doh.cleanbrowsing.org",
        "freedns.controld.com",
        "wikimedia-dns.org",
        "cloudflare-dns.com",
        "cloudflare-ech.com",
        "encryptedsni.com",
        "cloudflareaccess.com",
        "cloudflareapps.com",
        "cloudflarebolt.com",
        "cloudflareclient.com",
        "cloudflareinsights.com",
        "cloudflareok.com",
        "cloudflarepartners.com",
        "cloudflareportal.com",
        "cloudflarepreview.com",
        "cloudflareresolve.com",
        "cloudflaressl.com",
        "cloudflarestatus.com",
        "cloudflarestorage.com",
        "cloudflarestream.com",
        "cloudflaretest.com",
        "cloudfront.net",
        "zendesk.com",
        "frankerfacez.com",
        "ffzap.com",
        "betterttv.net",
        "7tv.app",
        "7tv.io",
        "localizeapi.com",
        "klipy.com",
        "live-video.net",
        "rutracker.org",
        "rutracker.cc",
        "rutracker.net",
        "rutor.info",
        "rutor.is",
        "nnmclub.to",
        "kinozal.tv",
        "soundcloud.com",
        "sndcdn.com",
        "medium.com",
        "patreon.com",
        "patreonusercontent.com",
        "proton.me",
        "protonmail.com",
        "archive.org"
      ],
      "icon": "list",
      "id": "general",
      "ip_id_zero": false,
      "name": "Прочие заблокированные",
      "strategies": [
        "alt11",
        "fake_multidisorder",
        "alt7",
        "seqovl_4pda",
        "multisplit_host",
        "http_hostcase",
        "faked_split_ts",
        "host_fake_split",
        "faked_disorder_ts",
        "fake_noack_split"
      ],
      "tcp_ports": [
        80,
        443
      ],
      "udp_ports": []
    },
    {
      "description": "Instagram, Facebook, X/Twitter. Часто блокируются по IP — может не помочь.",
      "domains": [
        "instagram.com",
        "cdninstagram.com",
        "facebook.com",
        "fbcdn.net",
        "fb.com",
        "messenger.com",
        "x.com",
        "twitter.com",
        "twimg.com",
        "t.co",
        "threads.net",
        "linkedin.com",
        "licdn.com"
      ],
      "icon": "globe",
      "id": "social",
      "ip_id_zero": false,
      "name": "Соцсети",
      "strategies": [
        "alt11",
        "fake_multidisorder",
        "alt7",
        "seqovl_4pda",
        "faked_split_ts",
        "host_fake_split",
        "faked_disorder_ts",
        "fake_noack_split"
      ],
      "tcp_ports": [
        443
      ],
      "udp_ports": []
    },
    {
      "description": "Только сайты из вашего списка («Списки доменов»). Пустой список — ничего не делает.",
      "domains": [],
      "icon": "globe",
      "id": "universal",
      "ip_id_zero": false,
      "name": "Свои домены",
      "strategies": [
        "alt11",
        "fake_multidisorder",
        "alt7",
        "seqovl_4pda",
        "multisplit_host",
        "http_hostcase",
        "faked_split_ts",
        "host_fake_split",
        "faked_disorder_ts",
        "fake_noack_split"
      ],
      "tcp_ports": [
        80,
        443
      ],
      "udp_ports": [
        [
          443,
          443
        ]
      ]
    }
  ],
  "strategies": [
    {
      "description": "Разрез на 2-м байте и сразу после начала SNI, наложение 679 байт фейка к google.com (general ALT7).",
      "id": "alt7",
      "name": "Split + Overlap (ALT7)",
      "steps": [
        {
          "at": [
            {
              "add": 2,
              "anchor": "start"
            },
            {
              "add": 1,
              "anchor": "sni_ext"
            }
          ],
          "kind": "multi_split"
        },
        {
          "kind": "seq_overlap",
          "len": 679,
          "sni": "www.google.com"
        }
      ]
    },
    {
      "description": "8 фейков, которые сервер отбросит по timestamp, затем наложение 681 байт (general ALT11).",
      "id": "alt11",
      "name": "Fake TS + Overlap (ALT11)",
      "steps": [
        {
          "fooling": [
            "ts"
          ],
          "kind": "fake",
          "repeats": 8,
          "sni": {
            "mode": "fixed",
            "value": "www.google.com"
          },
          "ttl": 0
        },
        {
          "at": [
            {
              "add": 1,
              "anchor": "start"
            }
          ],
          "kind": "multi_split"
        },
        {
          "kind": "seq_overlap",
          "len": 681,
          "sni": "www.google.com"
        }
      ]
    },
    {
      "description": "6 фейковых ClientHello к google.com с уменьшенным timestamp (general ALT10).",
      "id": "fake_ts",
      "name": "Fake TS",
      "steps": [
        {
          "fooling": [
            "ts"
          ],
          "kind": "fake",
          "repeats": 6,
          "sni": {
            "mode": "fixed",
            "value": "www.google.com"
          },
          "ttl": 0
        }
      ]
    },
    {
      "description": "Перед данными — 681 байт фейкового ClientHello к google.com «из прошлого». Основной метод zapret для YouTube.",
      "id": "seqovl_google",
      "name": "Overlap · Google",
      "steps": [
        {
          "at": [
            {
              "add": 1,
              "anchor": "start"
            }
          ],
          "kind": "multi_split"
        },
        {
          "kind": "seq_overlap",
          "len": 681,
          "sni": "www.google.com"
        }
      ]
    },
    {
      "description": "То же наложение, 568 байт к 4pda.to. Метод zapret для Discord и остальных сайтов.",
      "id": "seqovl_4pda",
      "name": "Overlap · 4pda",
      "steps": [
        {
          "at": [
            {
              "add": 1,
              "anchor": "start"
            }
          ],
          "kind": "multi_split"
        },
        {
          "kind": "seq_overlap",
          "len": 568,
          "sni": "4pda.to"
        }
      ]
    },
    {
      "description": "11 фейков с неверным seq, потом части задом наперёд (разрез посередине домена).",
      "id": "fake_multidisorder",
      "name": "Fake BadSeq + Disorder",
      "steps": [
        {
          "fooling": [
            "bad_seq"
          ],
          "kind": "fake",
          "repeats": 11,
          "sni": {
            "mode": "fixed",
            "value": "www.google.com"
          },
          "ttl": 0
        },
        {
          "at": [
            {
              "add": 1,
              "anchor": "start"
            },
            {
              "add": 0,
              "anchor": "midsld"
            }
          ],
          "kind": "multi_disorder"
        }
      ]
    },
    {
      "description": "Режет ClientHello посередине SNI.",
      "id": "split_mid",
      "name": "Split Mid-Host",
      "steps": [
        {
          "at": {
            "add": 0,
            "anchor": "host_mid"
          },
          "kind": "split"
        }
      ]
    },
    {
      "description": "Режет после 1 байта и отправляет части задом наперёд.",
      "id": "disorder_1",
      "name": "Disorder",
      "steps": [
        {
          "at": {
            "add": 1,
            "anchor": "start"
          },
          "kind": "disorder"
        }
      ]
    },
    {
      "description": "Режет перед, в середине и после имени сайта.",
      "id": "multisplit_host",
      "name": "Multi-Split Host",
      "steps": [
        {
          "at": [
            {
              "add": 1,
              "anchor": "start"
            },
            {
              "add": 0,
              "anchor": "host_start"
            },
            {
              "add": 0,
              "anchor": "host_mid"
            },
            {
              "add": 0,
              "anchor": "host_end"
            }
          ],
          "kind": "multi_split"
        }
      ]
    },
    {
      "description": "Сначала поддельный ClientHello с малым TTL, потом настоящий с разрезом.",
      "id": "fake_ttl_split",
      "name": "Fake TTL + Split",
      "steps": [
        {
          "fooling": [
            "ttl"
          ],
          "kind": "fake",
          "repeats": 2,
          "sni": {
            "mode": "random"
          },
          "ttl": 4
        },
        {
          "at": {
            "add": 0,
            "anchor": "host_mid"
          },
          "kind": "split"
        }
      ]
    },
    {
      "description": "Фейк с неверным seq и отправка частей задом наперёд.",
      "id": "fake_badseq_disorder",
      "name": "Fake BadSeq + Reverse",
      "steps": [
        {
          "fooling": [
            "bad_seq"
          ],
          "kind": "fake",
          "repeats": 3,
          "sni": {
            "mode": "random"
          },
          "ttl": 0
        },
        {
          "at": {
            "add": 0,
            "anchor": "host_mid"
          },
          "kind": "disorder"
        }
      ]
    },
    {
      "description": "Фейк к google.com, затем ClientHello режется на 1-м байте, и каждый кусок окружён фейком той же длины (fooling=ts).",
      "id": "faked_split_ts",
      "name": "Faked Split TS",
      "steps": [
        {
          "fooling": [
            "ts"
          ],
          "kind": "fake",
          "repeats": 6,
          "sni": {
            "mode": "fixed",
            "value": "www.google.com"
          },
          "ttl": 0
        },
        {
          "at": {
            "add": 1,
            "anchor": "start"
          },
          "disorder": false,
          "fooling": [
            "ts"
          ],
          "kind": "faked_split",
          "ttl": 0
        }
      ]
    },
    {
      "description": "Разрез посередине домена, куски задом наперёд, вокруг каждого — фейки той же длины, которые сервер отбросит по timestamp.",
      "id": "faked_disorder_ts",
      "name": "Faked Disorder TS",
      "steps": [
        {
          "at": {
            "add": 0,
            "anchor": "midsld"
          },
          "disorder": true,
          "fooling": [
            "ts"
          ],
          "kind": "faked_split",
          "ttl": 0
        }
      ]
    },
    {
      "description": "Имя сайта уходит отдельным куском, а до и после него — фейки с выдуманным именем той же длины.",
      "id": "host_fake_split",
      "name": "Host Fake Split",
      "steps": [
        {
          "fooling": [
            "ts"
          ],
          "kind": "host_fake_split",
          "ttl": 0
        }
      ]
    },
    {
      "description": "Фейки без флага ACK (сервер их отбросит, DPI — нет), затем разрез посередине имени.",
      "id": "fake_noack_split",
      "name": "Fake NoACK + Split",
      "steps": [
        {
          "fooling": [
            "data_no_ack"
          ],
          "kind": "fake",
          "repeats": 4,
          "sni": {
            "mode": "fixed",
            "value": "www.google.com"
          },
          "ttl": 0
        },
        {
          "at": {
            "add": 0,
            "anchor": "host_mid"
          },
          "kind": "split"
        }
      ]
    },
    {
      "description": "Для обычного HTTP: hoSt вместо Host + разрез.",
      "id": "http_hostcase",
      "name": "HTTP Host Case",
      "steps": [
        {
          "kind": "host_case"
        },
        {
          "at": {
            "add": 0,
            "anchor": "host_mid"
          },
          "kind": "split"
        }
      ]
    }
  ],
  "version": "0.4.0"
};
