# オブシリアンくん — second-brain Vault の番人

## 目的
Obsidian Vault（second-brain）の管理を1画面にまとめるTauriデスクトップアプリ。
①俯瞰マップ ②健康診断 ③Inbox振り分け の3タブ。

## 設計
- Tauri v2 + React + Vite + TypeScript。ポート **1500**（HMR 1501）
- Vaultパスは `src-tauri/src/lib.rs` の `vault_root()` にハードコード:
  `~/Library/Mobile Documents/iCloud~md~obsidian/Documents/second-brain`
- Rustコマンド: `vault_overview` / `health_check` / `list_inbox` / `list_dest_folders` / `read_note` / `move_note`
- 健康診断は全md（約2.4万件）をwalkdirで走査。リンク切れはファイル名（stem）集合と突き合わせ

## 決定事項
- 移動できるのは **00_Inbox配下のみ**。移動先はVault直下フォルダ（_meta / 80_Assets除外）
- 移動はファイル名を変えないので [[リンク]] は壊れない。リネーム機能は付けない
- 移動ログは `_meta/整理ログ_オブシリアンくん.md` に追記
- 保護フォルダ（wiki / 記事アーカイブ / メディア文字起こし / 20_知識ネットワーク / 99_Archive）は🔒バッジ表示。Vault側ルールの正は Vault直下の CLAUDE.md
- ゴミ掃除: 起動時＋手動ボタンで「無題のファイル/フォルダ（実質空のみ）・0バイトmd（保護フォルダ除く）」を削除。中身があるものは⚠️要確認表示のみ。削除は必ず整理ログに追記

## 罠
- Vault は iCloud 配下（パスにスペースあり）。`~/Documents/second-brain` は存在しない
- 記事アーカイブの記事タイトルにはスペースが正当に含まれるため、命名チェックから保護フォルダは除外（wikiは対象）
