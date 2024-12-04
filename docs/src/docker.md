# Docker

As of version 0.1.10, odd-box can find and route traffic to your locally running docker containers!

In order for this to work, you need to add the label `odd_box_port`, specifying the **private/internal** port to forward traffic to.

You can also set the `odd_box_host_name` label to whichever dns name you want to use for this container.
If the `odd_box_host_name` is not set, odd-box will default to use `<container-name>.odd-box.localhost`.
