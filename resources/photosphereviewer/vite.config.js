import { defineConfig } from 'vite'
import htmlMinify from 'vite-plugin-html-minify'
import { viteSingleFile } from "vite-plugin-singlefile"

export default defineConfig({
  plugins: [
    viteSingleFile(),
    htmlMinify({
      collapseWhitespace: true,
      removeComments: true,
      minifyCSS: true,
      minifyJS: true,
      removeRedundantAttributes: true,
      useShortDoctype: true,
      removeEmptyAttributes: true,
      removeStyleLinkTypeAttributes: true,
      removeScriptTypeAttributes: true,
      keepClosingSlash: true
    })
  ],
})