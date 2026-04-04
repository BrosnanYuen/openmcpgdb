#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include "maze.h"
#include "simulator.h"
#include "robot.h"
#include "fsm.h"

typedef void (*AppFunc)(void);

static Simulator g_sim;
static Robot g_robot;
static Fsm g_fsm;

static void app_init(void) {
    int rows = 21;
    int cols = 21;
    unsigned int seed = (unsigned int)time(NULL);

    printf("=== Maze Finder Robot ===\n");
    printf("Generating maze (%dx%d) with seed: %u\n\n", rows, cols, seed);

    simulator_init(&g_sim, rows, cols, seed);
    robot_init(&g_robot, &g_sim);
    fsm_init(&g_fsm, fsm_get_state_idle(), &g_robot);
}

static void app_run(void) {
    int max_steps = 10000;
    int step = 0;

    while (!simulator_is_at_goal(&g_sim) && step < max_steps) {
        printf("\n--- Step %d ---\n", step + 1);
        fsm_run(&g_fsm);
        simulator_print(&g_sim);
        step++;
    }

    if (simulator_is_at_goal(&g_sim)) {
        printf("\n*** Maze solved in %d steps! ***\n", g_sim.steps);
    } else {
        printf("\n*** Failed to solve maze within %d steps ***\n", max_steps);
    }
}

static void app_cleanup(void) {
    printf("\n=== Simulation Complete ===\n");
}

int main(void) {
    AppFunc funcs[] = {app_init, app_run, app_cleanup};

    for (int i = 0; i < 3; i++) {
        funcs[i]();
    }

    return 0;
}
