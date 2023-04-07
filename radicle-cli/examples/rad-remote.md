`rad remote` can help with managing git's radicle remotes.

Let's take a look at the radicle remotes we have from cloning a project.

```
$ rad remote -v
rad	rad://z42hL2jL4XNk6K8oHQaSWfMgCL7ji/z6Mkt67GdsW7715MEfRuP4pSZxJRJh6kj6Y48WRqVv4N1tRk (fetch)
z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi	rad://z42hL2jL4XNk6K8oHQaSWfMgCL7ji/z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi (fetch)
```

The remote `rad` is our fork of the project, and
'z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi' is the origin's.

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
z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi
```

When we finish to do what we need to do with we the bob remote, we can remove it

```
$ rad remote rm bob
🗑️ Remote bob removed
```
