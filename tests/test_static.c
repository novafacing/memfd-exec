#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

/* Listen on port 1234 for a connection and print out the data received. */
int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "Usage: %s <port>\n", argv[0]);
  }
  int port = atoi(argv[1]);
  int sock = socket(AF_INET, SOCK_STREAM, 0);
  struct sockaddr_in addr;
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  addr.sin_addr.s_addr = INADDR_ANY;
  (void)bind(sock, (struct sockaddr *)&addr, sizeof(addr));
  listen(sock, 1);
  int client = accept(sock, NULL, NULL);
  char buf[1024];
  int len = read(client, buf, sizeof(buf));
  write(STDOUT_FILENO, buf, len);
  close(client);
  close(sock);
  return 0;
}