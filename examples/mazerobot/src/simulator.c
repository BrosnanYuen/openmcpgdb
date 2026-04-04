#include "simulator.h"
#include <stdio.h>

void simulator_init(Simulator *sim, int rows, int cols, unsigned int seed) {
    maze_init(&sim->maze, rows, cols);
    maze_generate(&sim->maze, seed);
    sim->robot_row = sim->maze.start_row;
    sim->robot_col = sim->maze.start_col;
    sim->steps = 0;
    sim->solved = false;
}

bool simulator_move_robot(Simulator *sim, int new_row, int new_col) {
    if (!maze_is_valid_move(&sim->maze, new_row, new_col)) {
        return false;
    }

    sim->robot_row = new_row;
    sim->robot_col = new_col;
    sim->steps++;

    if (sim->robot_row == sim->maze.end_row && sim->robot_col == sim->maze.end_col) {
        sim->solved = true;
    }

    return true;
}

void simulator_reset(Simulator *sim) {
    sim->robot_row = sim->maze.start_row;
    sim->robot_col = sim->maze.start_col;
    sim->steps = 0;
    sim->solved = false;
}

void simulator_print_status(const Simulator *sim) {
    printf("\n=== Robot Status ===\n");
    printf("Position: (%d, %d)\n", sim->robot_row, sim->robot_col);
    printf("Steps: %d\n", sim->steps);
    printf("Goal: %s\n", sim->solved ? "REACHED" : "Not reached");
    printf("====================\n");
}

void simulator_print(const Simulator *sim) {
    for (int r = 0; r < sim->maze.rows; r++) {
        for (int c = 0; c < sim->maze.cols; c++) {
            if (r == sim->robot_row && c == sim->robot_col) {
                printf("R");
            } else {
                const char *symbols[] = {"#", ".", "S", "E", "o"};
                printf("%s", symbols[sim->maze.grid[r][c]]);
            }
        }
        printf("\n");
    }
}

bool simulator_is_at_goal(const Simulator *sim) {
    return sim->solved;
}
