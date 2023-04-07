Now lets add a bob as a new remote

```
$ rad remote add did:key:z6Mkt67GdsW7715MEfRuP4pSZxJRJh6kj6Y48WRqVv4N1tRk --alias bob
❲🚀❳ Remote bob added with rad://z42hL2jL4XNk6K8oHQaSWfMgCL7ji/z6Mkt67GdsW7715MEfRuP4pSZxJRJh6kj6Y48WRqVv4N1tRk
```

Now, we can see that there is a new new remote in the list of remotes

```
$ rad remote list
bob
rad
```

You can see both bob and rad as remotes.  The remote `rad` is our remote of the project.

When we finish to do what we need to do with we the bob remote, we can remove it

```
$ rad remote rm bob
🗑️ Remote bob removed
```
