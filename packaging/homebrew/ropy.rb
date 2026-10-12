cask "ropy" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.5.7"
  sha256 arm:   "957bc3d920629aee4eda8625d9c48ca926bf4a77bc05ca752ff694088fc8a17c",
         intel: "3a9fedac77e6a895e2dcea1ed8dc35105cf220a90aee8451cfeac94fc2918032"

  url "https://github.com/StudentWeis/ropy/releases/download/#{version}/ropy-#{arch}-apple-darwin.dmg"
  name "Ropy"
  desc "Clipboard manager built with Rust and GPUI"
  homepage "https://github.com/StudentWeis/ropy"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on :macos

  app "Ropy.app"

  # Cask moves the bundle to /Applications, outside the Homebrew prefix.
  # Use Ropy's existing publisher marker to leave upgrades to Homebrew.
  postflight_steps do
    touch "Ropy.app/Contents/.ropy-managed", base: :appdir
  end

  uninstall quit: "com.ropy.app"
end
