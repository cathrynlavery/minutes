import type { Metadata } from "next";
import { Analytics } from "@vercel/analytics/next";
import "./globals.css";

export const metadata: Metadata = {
  title: "minutes — open-source conversation memory",
  description:
    "Record meetings, capture voice memos, search everything. Local transcription with whisper.cpp, structured markdown, Claude-native. Free forever.",
  metadataBase: new URL("https://useminutes.app"),
  alternates: { canonical: "/" },
  icons: {
    icon: [
      { url: "/favicon.svg", type: "image/svg+xml" },
    ],
  },
  openGraph: {
    title: "minutes — open-source conversation memory",
    description:
      "Record meetings, capture voice memos, ask your AI what was decided. Local transcription, structured markdown, free forever.",
    type: "website",
    url: "https://useminutes.app",
    siteName: "minutes",
  },
  twitter: {
    card: "summary",
    title: "minutes — open-source conversation memory",
    description:
      "Record meetings, capture voice memos, ask your AI what was decided. Local, free, MIT licensed.",
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          rel="stylesheet"
          href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500&family=Instrument+Serif:ital@0;1&display=swap"
        />
        <link rel="alternate" type="text/plain" href="/llms.txt" />
        <meta
          name="theme-color"
          media="(prefers-color-scheme: light)"
          content="#F8F4ED"
        />
        <meta
          name="theme-color"
          media="(prefers-color-scheme: dark)"
          content="#0D0D0B"
        />
      </head>
      <body className="font-sans antialiased">
        {children}
        <Analytics />
      </body>
    </html>
  );
}
