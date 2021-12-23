# This Documentation

This documentation is generated using mdbook. you can check out the [mdbook documentation](https://rust-lang.github.io/mdBook/) for more info on mdbook.

TODO, update after deciding the final way to host the mdbook
To compile and deploy this mdbook follow the instructions

```
git worktree add /tmp/book -b gh-pages
mdbook build
rm -rf /tmp/book/* # this won't delete the .git directory
cp -rp book/* /tmp/book/
cd /tmp/book
git add -A
git commit 'new book message'
git push origin gh-pages
cd -
```
