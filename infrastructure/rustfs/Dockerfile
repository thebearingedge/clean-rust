FROM index.docker.io/rustfs/rc:v0.1.36 AS rc

FROM index.docker.io/rustfs/rustfs:1.0.4

COPY --from=rc /usr/bin/rc /usr/bin/rc
