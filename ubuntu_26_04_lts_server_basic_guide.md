# Ubuntu 26.04 LTS Server 基本操作ガイド

> コードネーム: Resolute Raccoon  
> リリース: 2026年4月  
> 特徴: Rustベースのツール採用（sudo-rs、uutils/coreutils）

---

## 1. コマンドの打ち方

### 基本構文

```bash
コマンド [オプション] [引数]
```

| 要素 | 説明 | 例 |
|------|------|-----|
| **コマンド** | 実行するプログラム名 | `ls`, `cat`, `sudo` |
| **オプション** | コマンドの動作を変更するフラグ | `-l`, `--help` |
| **引数** | コマンドの対象となるファイルやパス | `/var/log/syslog` |

### 具体例

```bash
ls -la /var/log/
#  ls    → コマンド（ファイル一覧表示）
#  -la   → オプション（-l:詳細表示, -a:隠しファイル含む）
#  /var/log/ → 引数（対象ディレクトリ）
```

### ⚠️ 26.04特有の変更

- **sudo-rs**がデフォルト：従来の`sudo`と同じように使えますが、内部がRustで書き直されています
- **uutils/coreutils**：`ls`, `cp`, `mv`, `cat`などの基本コマンドがRust実装に移行。挙動はほぼ同じですが、GNU版との互換性は約88%です
- **明示的なオプション推奨**：`head -20`のような省略形ではなく、`head -n 20`のように明示的に書くことを推奨

---

## 2. コマンドの種類（よく使うもの）

### システム管理

| コマンド | 用途 |
|----------|------|
| `sudo` | 管理者権限で実行（26.04ではsudo-rs） |
| `systemctl` | systemdサービスの管理（起動/停止/確認） |
| `journalctl` | システムログの閲覧 |
| `apt` | パッケージ管理（26.04ではAPT 3） |
| `snap` | Snapパッケージ管理 |
| `ufw` | ファイアウォール設定 |
| `ip` | ネットワーク設定確認 |
| `ss` | ソケット（ポート）状態確認 |
| `useradd` / `adduser` | ユーザー作成 |
| `passwd` | パスワード変更 |

### ファイル操作

| コマンド | 用途 |
|----------|------|
| `ls` | ファイル一覧 |
| `cd` | ディレクトリ移動 |
| `pwd` | 現在のディレクトリ表示 |
| `cat` | ファイル内容表示 |
| `less` | ページャーで閲覧 |
| `cp` | コピー |
| `mv` | 移動/リネーム |
| `rm` | 削除 |
| `mkdir` | ディレクトリ作成 |
| `chmod` | パーミッション変更 |
| `chown` | 所有者変更 |
| `find` | ファイル検索 |
| `grep` | テキスト検索 |

### テキスト処理

| コマンド | 用途 |
|----------|------|
| `head -n 20` | 先頭20行表示 |
| `tail -n 50` | 末尾50行表示 |
| `tail -f` | リアルタイム追跡 |
| `awk` | フィールド抽出 |
| `sed` | 文字列置換 |
| `wc -l` | 行数カウント |
| `sort` | ソート |
| `uniq` | 重複除去 |

### ネットワーク

| コマンド | 用途 |
|----------|------|
| `ping -c 4` | 疎通確認 |
| `curl` | HTTPリクエスト |
| `wget` | ファイルダウンロード |
| `ssh` | リモート接続 |
| `scp` | リモートファイル転送 |
| `netplan` | ネットワーク設定 |

---

## 3. コマンドのオプション

### オプションの書き方

```bash
# 短い形式（1文字、複数まとめられる）
ls -l -a -h        # 個別指定
ls -lah            # まとめて指定

# 長い形式（読みやすい）
ls --all --human-readable

# 引数を持つオプション
sudo -u deploy     # -uの後にユーザー名
tail -n 50         # -nの後に行数
```

### 主要コマンドの代表的なオプション

#### `apt`（パッケージ管理）

```bash
sudo apt update              # パッケージリスト更新
sudo apt upgrade             # パッケージ更新（削除なし）
sudo apt full-upgrade        # 依存関係解決を含む更新
sudo apt install <package>   # インストール
sudo apt remove <package>    # 削除（設定残す）
sudo apt purge <package>     # 完全削除
sudo apt autoremove          # 不要パッケージ削除
sudo apt search <name>       # 検索
sudo apt --fix-broken install # 依存関係修復
```

#### `systemctl`（サービス管理）

```bash
systemctl status <service>   # 状態確認
sudo systemctl start <service>   # 起動
sudo systemctl stop <service>    # 停止
sudo systemctl restart <service> # 再起動
sudo systemctl enable <service>  # 自動起動有効化
sudo systemctl disable <service> # 自動起動無効化
```

#### `journalctl`（ログ閲覧）

```bash
journalctl -u <service> -n 50 --no-pager  # 最新50行
journalctl -u <service> -f                # リアルタイム追跡
journalctl --since "1 hour ago"         # 直近1時間
```

#### `ufw`（ファイアウォール）

```bash
sudo ufw status verbose      # 状態確認
sudo ufw allow OpenSSH       # SSH許可
sudo ufw allow 80,443/tcp    # ポート許可
sudo ufw default deny incoming  # デフォルト拒否
sudo ufw enable              # 有効化
```

---

## 4. コマンドの止め方

### 実行中のコマンドを停止

| 方法 | キー | 用途 |
|------|------|------|
| **強制終了** | `Ctrl + C` | 実行中のプロセスを即座に停止 |
| **一時停止** | `Ctrl + Z` | 実行を一時停止（バックグラウンドに移動） |
| **ログアウト** | `Ctrl + D` | シェルからログアウト（EOFを送信） |

### バックグラウンド操作

```bash
# 一時停止したジョブをバックグラウンドで再開
bg

# フォアグラウンドで再開
fg

# ジョブ一覧確認
jobs

# コマンドをバックグラウンドで実行
long_command &
```

### 別のターミナルからプロセスを停止

```bash
# プロセスIDを確認
ps aux | grep <プロセス名>
# または
pgrep -af '<プロセス名>'

# プロセスを停止
kill <PID>          # 通常終了（SIGTERM）
kill -9 <PID>       # 強制終了（SIGKILL）
sudo killall <name> # 名前で一括停止
```

### サービスの停止

```bash
sudo systemctl stop <service>     # 停止
sudo systemctl restart <service>  # 再起動
sudo systemctl disable <service>  # 自動起動無効化
```

---

## 5. 基本パッケージではないが必須パッケージ

Ubuntu Serverの最小インストールには含まれていないが、実運用で必須となるパッケージです。

### セキュリティ関連

```bash
sudo apt install -y unattended-upgrades   # 自動セキュリティ更新
sudo apt install -y fail2ban              # SSHブルートフォース攻撃防止
sudo apt install -y ufw                   # ファイアウォール（最小インストールでは未含の場合あり）
```

### 監視・管理ツール

```bash
sudo apt install -y htop                  # 対話的プロセスビューア
sudo apt install -y iotop                 # I/O監視
sudo apt install -y ncdu                  # ディスク使用量可視化
sudo apt install -y tree                  # ディレクトリツリー表示
```

### ネットワークツール

```bash
sudo apt install -y curl                  # HTTPリクエスト
sudo apt install -y wget                  # ファイルダウンロード
sudo apt install -y net-tools             # ifconfig, netstatなど（レガシーだが便利）
sudo apt install -y dnsutils              # dig, nslookup
```

### エディタ

```bash
sudo apt install -y vim                   # 高機能テキストエディタ
# または
sudo apt install -y nano                  # シンプルなエディタ
```

### 開発・ビルドツール

```bash
sudo apt install -y build-essential       # gcc, g++, makeなど
sudo apt install -y git                   # バージョン管理
```

### 初期セットアップで推奨される一括インストール

```bash
# 基本セキュリティ＋ユーティリティ
sudo apt update && sudo apt full-upgrade -y
sudo apt install -y \
    unattended-upgrades \
    fail2ban \
    ufw \
    htop \
    curl \
    wget \
    vim \
    git \
    net-tools \
    dnsutils
```

---

## 補足：初期セットアップの流れ

新規サーバー構築時の推奨手順です：

### 1. パッチ適用

```bash
sudo apt update && sudo apt full-upgrade -y
```

### 2. sudoユーザー作成

```bash
sudo adduser deploy
sudo usermod -aG sudo deploy
```

### 3. SSH鍵認証設定

```bash
# 公開鍵をコピー
rsync --archive --chown=deploy:deploy ~/.ssh /home/deploy
```

### 4. SSHハードニング

```bash
# /etc/ssh/sshd_config.d/99-hardening.conf
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
sudo systemctl restart ssh
```

### 5. ファイアウォール有効化

```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow OpenSSH
sudo ufw allow 80,443/tcp
sudo ufw enable
```

### 6. 自動セキュリティ更新

```bash
sudo apt install -y unattended-upgrades
sudo dpkg-reconfigure -plow unattended-upgrades
```

---

## まとめ

Ubuntu 26.04 LTS Serverは、Rustベースのツール採用によりメモリ安全性が向上した現代的なサーバーOSです。基本操作は従来のUbuntuと同じですが、コマンドのオプションは明示的な形式（例：`head -n 20`）を使うことを意識するとよいでしょう。
