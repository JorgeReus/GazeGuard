package com.lemiosto.workouttracker

interface Platform {
    val name: String
}

expect fun getPlatform(): Platform