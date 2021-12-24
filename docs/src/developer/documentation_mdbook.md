# This Documentation

This documentation is created using mdbook and aims to provide a concise reference for users and developers of youki. For more information on mdbook itself, you can check out the [mdbook documentation](https://rust-lang.github.io/mdBook/).

Please make sure that you update this documentation along with newly added features and resources that you found helpful while developing, so that it will be helpful for newcomers.

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
