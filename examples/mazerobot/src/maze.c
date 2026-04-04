#include "maze.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int dr[] = {-1, 1, 0, 0};
static int dc[] = {0, 0, -1, 1};

void maze_init(Maze *maze, int rows, int cols) {
    maze->rows = rows;
    maze->cols = cols;
    maze->start_row = 1;
    maze->start_col = 1;
    maze->end_row = rows - 2;
    maze->end_col = cols - 2;

    for (int r = 0; r < rows; r++) {
        for (int c = 0; c < cols; c++) {
            maze->grid[r][c] = CELL_WALL;
        }
    }
}

static void carve_passage(Maze *maze, int row, int col, bool visited[MAZE_MAX_SIZE][MAZE_MAX_SIZE], unsigned int *state) {
    visited[row][col] = true;
    maze->grid[row][col] = CELL_PATH;

    int dirs[4] = {0, 1, 2, 3};

    for (int i = 3; i > 0; i--) {
        int j = (*state) % (i + 1);
        (*state) = (*state) * 1103515245 + 12345;
        int temp = dirs[i];
        dirs[i] = dirs[j];
        dirs[j] = temp;
    }

    for (int i = 0; i < 4; i++) {
        int nr = row + dr[dirs[i]] * 2;
        int nc = col + dc[dirs[i]] * 2;

        if (nr > 0 && nr < maze->rows - 1 && nc > 0 && nc < maze->cols - 1 && !visited[nr][nc]) {
            maze->grid[row + dr[dirs[i]]][col + dc[dirs[i]]] = CELL_PATH;
            carve_passage(maze, nr, nc, visited, state);
        }
    }
}

void maze_generate(Maze *maze, unsigned int seed) {
    bool visited[MAZE_MAX_SIZE][MAZE_MAX_SIZE];
    memset(visited, 0, sizeof(visited));

    unsigned int state = seed;
    carve_passage(maze, 1, 1, visited, &state);

    maze->grid[maze->start_row][maze->start_col] = CELL_START;
    maze->grid[maze->end_row][maze->end_col] = CELL_END;
}

void maze_print(const Maze *maze) {
    const char *symbols[] = {"#", ".", "S", "E", "o"};

    for (int r = 0; r < maze->rows; r++) {
        for (int c = 0; c < maze->cols; c++) {
            printf("%s", symbols[maze->grid[r][c]]);
        }
        printf("\n");
    }
}

bool maze_is_valid_move(const Maze *maze, int row, int col) {
    if (row < 0 || row >= maze->rows || col < 0 || col >= maze->cols) {
        return false;
    }
    CellType cell = maze->grid[row][col];
    return cell == CELL_PATH || cell == CELL_END;
}

void maze_set_start(Maze *maze, int row, int col) {
    maze->start_row = row;
    maze->start_col = col;
}

void maze_set_end(Maze *maze, int row, int col) {
    maze->end_row = row;
    maze->end_col = col;
}
