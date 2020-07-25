# cleanup-history-rs

Filters my `.bash_history` through a set of regexes, deduplicates, and sorts it
by most recently used.

Based on https://github.com/naggie/dotfiles/blob/master/scripts/cleanup-history

## Notes on `.bash_history`

Format:

```plaintext
#1593575811
echo each command has a timestamp immediately before it
#2
#1593575811
echo 'after multiple timestamp lines, `history` will show the timestamp 1593575811'
#1593575811
#3
echo after multiple timestamp lines, this will show the timestamp 3
#1
echo 'when you run `history` this will show up with a timestamp long ago but still at the end of the list'
#1593575811
echo this will have the same timestamp as others above, duplicates don\'t matter
#1593575812
#1593575813
#1593575814
#1593575815
#1593575816
#1593575817
#1593575818
#1593575819
#1593575820
#1593575821
echo 'once you `history -w` all these extra timestamps will get removed'
#1593576854
for ((i=0;i<5;i++)); do echo $i; done
#1593576854
echo ^^ that was written on multiple lines
#1593576874
echo 'foo
bar'
#1593576874
echo ^^ that was also written on multiple lines, cmdhist=on, lithist=off
```

## Gotchas

If a line starts with `#\d+`, it will be interpreted as a timestamp.

```console
$ export HISTFILE=./foo
$ history -c
$ echo 'this
#1234
that'
$ history -w
$ cat foo
#1594044806
echo 'this
#1234
that'
#1594044814
history -w
$ history -c
$ history -r
$ history
    1  2020-07-06 08:16.14 | history -r
    2  2020-07-06 08:15.15 | echo 'this
    3  1969-12-31 17:20.34 | that'
    4  2020-07-06 08:15.25 | history -w
    5  2020-07-06 08:16.16 | history
```

```console
$ history -c
$ echo 'foo
#1234 bar
baz'
$ history -w
$ history # correct in memory
    1  2020-07-06 08:24.30 | echo 'foo
#1234 bar
baz'
    2  2020-07-06 08:24.38 | history -w
    3  2020-07-06 08:24.41 | history
$ cat foo
#1594045470
echo 'foo
#1234 bar
baz'
#1594045478
history -w
$ history -c # clear in-memory history
$ history -r # reread from file
$ history # now incorrectly interprets `#1234 bar` as a timestamp
    1  2020-07-06 08:19.49 | history -r
    2  2020-07-06 08:19.09 | echo 'foo
    3  1969-12-31 17:20.34 | baz'
    4  2020-07-06 08:19.31 | history -w
    5  2020-07-06 08:19.51 | history
```