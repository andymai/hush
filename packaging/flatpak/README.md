# Hush's Flatpak repository

With each release, `.github/workflows/flatpak.yml` publishes Hush's own Flatpak repository at <https://andymai.github.io/hush/>. Users add it through `hush.flatpakrepo`, and Bazaar and other Flatpak stores list Hush from it.

## The release jobs

1. `release.yml` starts `flatpak.yml` on `main` with the release tag. Only `main` may deploy to GitHub Pages.
2. The `flatpak` job builds the tag in the Flathub build image.
3. The `upload` job attests the bundle and uploads it to the release as `hush-<version>-x86_64.flatpak`.
4. The `repo` job runs `repo.sh`. The script pulls the last published commits, puts the new commit on top, and signs the commit and the summary. Then `repo-check.sh` installs Hush from the new repository on disk.
5. The `pages` job deploys the site. Each deploy replaces the whole site, so the site holds only the repository.

To publish a release again, run `gh workflow run flatpak.yml --ref main -f tag=<tag>`.

On pull requests and on `main`, the `repo` job signs with a throwaway key and publishes nothing. When two releases have a bundle, it also runs `repo-test.sh` on them, which publishes the older one, installs it, and updates to the newer one over a local server.

## The signing key

The `FLATPAK_SIGNING_KEY` secret signs the repository. It is an armored GPG secret key without a passphrase. Its public half is `hush-repo.asc` in this folder, with this fingerprint:

```text
55D8 DDBB DF55 1D71 DD9A  9013 E0F6 ACEA 036E AE96
```

A release stops when the secret is empty or when it holds another key.

Keep a copy of the secret key outside GitHub, for example in a password manager, because GitHub never shows a secret again.

## Setup

1. Make a key, and give it an empty passphrase:

   ```sh
   gpg --quick-gen-key "Hush Flatpak repository" rsa4096 sign never
   ```

2. Put its public half in this folder, with the fingerprint that gpg printed:

   ```sh
   gpg --armor --export FINGERPRINT > packaging/flatpak/hush-repo.asc
   ```

3. Store its secret half in the repository:

   ```sh
   gpg --armor --export-secret-keys FINGERPRINT | gh secret set FLATPAK_SIGNING_KEY
   ```

4. Turn on GitHub Pages, with GitHub Actions as its source. The `github-pages` environment must let `main` deploy.

   ```sh
   gh api -X POST repos/andymai/hush/pages -f build_type=workflow
   ```

## A new key

A client that added the remote trusts only the key in the `hush.flatpakrepo` that it read, and it refuses a commit that another key signed. So a new key needs these steps:

1. Do the setup steps with the new key, and merge the new `hush-repo.asc`.
2. Turn GitHub Pages off and on again. The next release checks the published commits with the new key and stops on the old ones. Without a published site, it starts a new repository.
3. Tell users to add the remote again: `flatpak remote-delete --force hush`, then the `remote-add` from INSTALL.md. With `--force`, the installed Hush stays, and it updates from the new remote of the same name.
