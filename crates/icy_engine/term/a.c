#include <stdio.h>

int main(void)
{
    int i, j, n;
    printf("Plain:\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Bold:\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[1;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }    printf("Bold:\n");
        for (i = 0; i < 11; i++) {
            for (j = 0; j < 10; j++) {
                n = 10 * i + j;
                if (n > 108) break;
                printf("\033[2;%dm %3d\033[m", n, n);
            }
            printf("\n");
        }

    printf("Italic:\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[3;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Blink:\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[5;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Underline:\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[4;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Crossed out :\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[9;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Double underlined :\n");
    for (i = 0; i < 11; i++) {
        for (j = 0; j < 10; j++) {
            n = 10 * i + j;
            if (n > 108) break;
            printf("\033[21;%dm %3d\033[m", n, n);
        }
        printf("\n");
    }

    printf("Conceal/hide :\n");
      for (i = 0; i < 11; i++) {
          for (j = 0; j < 10; j++) {
              n = 10 * i + j;
              if (n > 108) break;
              printf("\033[8;%dm %3d\033[m", n, n);
          }
          printf("\n");
      }

    return 0;
}
